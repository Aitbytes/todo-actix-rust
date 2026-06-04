use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer, Responder, get, post, put, delete};
use serde::{Deserialize, Serialize};
use sqlx::mysql::MySqlPoolOptions;
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow)]
struct Todo {
    id: i32,
    title: String,
    completed: bool,
    created_at: chrono::NaiveDateTime,
}

#[derive(Debug, Deserialize)]
struct NewTodo {
    title: String,
}

#[derive(Debug, Deserialize)]
struct UpdateTodo {
    title: Option<String>,
    completed: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ApiResponse {
    status: String,
    message: String,
}

#[get("/todos")]
async fn get_todos(pool: web::Data<sqlx::MySqlPool>) -> impl Responder {
    match sqlx::query_as::<_, Todo>("SELECT id, title, completed, created_at FROM todos ORDER BY created_at DESC")
        .fetch_all(pool.get_ref())
        .await
    {
        Ok(todos) => HttpResponse::Ok().json(todos),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse {
            status: "error".into(),
            message: format!("Failed to fetch todos: {}", e),
        }),
    }
}

#[post("/todos")]
async fn create_todo(pool: web::Data<sqlx::MySqlPool>, body: web::Json<NewTodo>) -> impl Responder {
    match sqlx::query("INSERT INTO todos (title) VALUES (?)")
        .bind(&body.title)
        .execute(pool.get_ref())
        .await
    {
        Ok(result) => {
            let id = result.last_insert_id() as i32;
            match sqlx::query_as::<_, Todo>("SELECT id, title, completed, created_at FROM todos WHERE id = ?")
                .bind(id)
                .fetch_one(pool.get_ref())
                .await
            {
                Ok(todo) => HttpResponse::Created().json(todo),
                Err(e) => HttpResponse::InternalServerError().json(ApiResponse {
                    status: "error".into(),
                    message: format!("Created but failed to fetch: {}", e),
                }),
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse {
            status: "error".into(),
            message: format!("Failed to create todo: {}", e),
        }),
    }
}

#[put("/todos/{id}")]
async fn update_todo(
    pool: web::Data<sqlx::MySqlPool>,
    path: web::Path<i32>,
    body: web::Json<UpdateTodo>,
) -> impl Responder {
    let id = path.into_inner();

    // Check exists
    let existing = sqlx::query_as::<_, Todo>("SELECT id, title, completed, created_at FROM todos WHERE id = ?")
        .bind(id)
        .fetch_optional(pool.get_ref())
        .await;

    match existing {
        Ok(Some(todo)) => {
            let title = body.title.clone().unwrap_or(todo.title);
            let completed = body.completed.unwrap_or(todo.completed);

            match sqlx::query("UPDATE todos SET title = ?, completed = ? WHERE id = ?")
                .bind(&title)
                .bind(completed)
                .bind(id)
                .execute(pool.get_ref())
                .await
            {
                Ok(_) => match sqlx::query_as::<_, Todo>("SELECT id, title, completed, created_at FROM todos WHERE id = ?")
                    .bind(id)
                    .fetch_one(pool.get_ref())
                    .await
                {
                    Ok(updated) => HttpResponse::Ok().json(updated),
                    Err(e) => HttpResponse::InternalServerError().json(ApiResponse {
                        status: "error".into(),
                        message: format!("Updated but failed to fetch: {}", e),
                    }),
                },
                Err(e) => HttpResponse::InternalServerError().json(ApiResponse {
                    status: "error".into(),
                    message: format!("Failed to update todo: {}", e),
                }),
            }
        }
        Ok(None) => HttpResponse::NotFound().json(ApiResponse {
            status: "error".into(),
            message: "Todo not found".into(),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse {
            status: "error".into(),
            message: format!("Database error: {}", e),
        }),
    }
}

#[delete("/todos/{id}")]
async fn delete_todo(pool: web::Data<sqlx::MySqlPool>, path: web::Path<i32>) -> impl Responder {
    let id = path.into_inner();

    match sqlx::query("DELETE FROM todos WHERE id = ?")
        .bind(id)
        .execute(pool.get_ref())
        .await
    {
        Ok(result) => {
            if result.rows_affected() > 0 {
                HttpResponse::Ok().json(ApiResponse {
                    status: "success".into(),
                    message: format!("Todo {} deleted", id),
                })
            } else {
                HttpResponse::NotFound().json(ApiResponse {
                    status: "error".into(),
                    message: "Todo not found".into(),
                })
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse {
            status: "error".into(),
            message: format!("Failed to delete todo: {}", e),
        }),
    }
}

#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok().json(ApiResponse {
        status: "ok".into(),
        message: "Todo API is running".into(),
    })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://todouser:TodoPass123@todo-db:3306/tododb".into());

    log::info!("Connecting to database...");

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to MariaDB");

    // Create table
    sqlx::query("DROP TABLE IF EXISTS todos")
        .execute(&pool)
        .await
        .expect("Failed to drop old todos table");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS todos (
            id INT AUTO_INCREMENT PRIMARY KEY,
            title VARCHAR(255) NOT NULL,
            completed BOOLEAN NOT NULL DEFAULT FALSE,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(&pool)
    .await
    .expect("Failed to create todos table");

    log::info!("Database connected and table ready");

    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
    log::info!("Starting server at {}", bind_addr);

    HttpServer::new(move || {
        let cors = Cors::permissive();
        App::new()
            .wrap(cors)
            .app_data(web::Data::new(pool.clone()))
            .service(health)
            .service(get_todos)
            .service(create_todo)
            .service(update_todo)
            .service(delete_todo)
    })
    .bind(&bind_addr)?
    .run()
    .await
}
