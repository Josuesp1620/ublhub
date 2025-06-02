use crate::{dto::{NewProjectDto, ProjectDto}, server::AppState};
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set};
use openubl_entity::project;

#[get("/projects")]
async fn list_projects(state: web::Data<AppState>) -> impl Responder {
    match project::Entity::find().all(&state.db).await {
        Ok(projects) => {
            let project_dtos: Vec<ProjectDto> =
                projects.into_iter().map(ProjectDto::from).collect();
            HttpResponse::Ok().json(project_dtos)
        }
        Err(e) => {
            log::error!("Failed to list projects: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, http::StatusCode};
    use std::sync::Arc;
    use openubl_api::system::InnerSystem;
    use crate::dto::{NewProjectDto, ProjectDto};
    use crate::server::configure; // Assuming 'configure' sets up all routes

    async fn setup_test_app() -> (impl actix_web::dev::Service<actix_http::Request, Response = actix_web::dev::ServiceResponse, Error = actix_web::Error>, Arc<AppState>) {
        let system = InnerSystem::for_test().await.unwrap();
        let app_state = Arc::new(AppState { system, storage: unimplemented!("Storage not needed for these project tests") });

        let app = test::init_service(
            App::new()
                .app_data(web::Data::from(app_state.clone()))
                .configure(configure) // Use the main configure function
        ).await;
        (app, app_state)
    }

    #[actix_web::test]
    async fn test_create_project_endpoint() {
        let (app, _app_state) = setup_test_app().await;

        let new_project = NewProjectDto {
            name: "Test API Project".to_string(),
            description: Some("API Test Description".to_string()),
        };

        let req = test::TestRequest::post()
            .uri("/projects")
            .set_json(&new_project)
            .to_request();

        let resp: ProjectDto = test::call_and_read_body_json(&app, req).await;

        assert_eq!(resp.name, "Test API Project");
        assert_eq!(resp.description, Some("API Test Description".to_string()));
        assert!(resp.id > 0);
    }

    #[actix_web::test]
    async fn test_list_projects_endpoint() {
        let (app, app_state) = setup_test_app().await;

        // Create a project directly via system for setup
        let project_am = openubl_entity::project::ActiveModel {
            name: sea_orm::Set("Project Alpha".to_string()),
            description: sea_orm::Set(Some("Alpha desc".to_string())),
            ..Default::default()
        };
        app_state.system.persist_project(&project_am, openubl_api::db::Transactional::None).await.unwrap();

        let project_am2 = openubl_entity::project::ActiveModel {
            name: sea_orm::Set("Project Beta".to_string()),
            description: sea_orm::Set(None),
            ..Default::default()
        };
        app_state.system.persist_project(&project_am2, openubl_api::db::Transactional::None).await.unwrap();
        
        let req = test::TestRequest::get().uri("/projects").to_request();
        let resp: Vec<ProjectDto> = test::call_and_read_body_json(&app, req).await;

        assert_eq!(resp.len(), 2);
        assert!(resp.iter().any(|p| p.name == "Project Alpha"));
        assert!(resp.iter().any(|p| p.name == "Project Beta"));
    }

    #[actix_web::test]
    async fn test_get_project_endpoint() {
        let (app, app_state) = setup_test_app().await;
        
        let project_am = openubl_entity::project::ActiveModel {
            name: sea_orm::Set("Specific Project".to_string()),
            description: sea_orm::Set(Some("Specific Desc".to_string())),
            ..Default::default()
        };
        let created_project_ctx = app_state.system.persist_project(&project_am, openubl_api::db::Transactional::None).await.unwrap();
        let project_id = created_project_ctx.project.id;

        // Test found
        let req_found = test::TestRequest::get()
            .uri(&format!("/projects/{}", project_id))
            .to_request();
        let resp_found: ProjectDto = test::call_and_read_body_json(&app, req_found).await;
        assert_eq!(resp_found.id, project_id);
        assert_eq!(resp_found.name, "Specific Project");

        // Test not found
        let req_not_found = test::TestRequest::get().uri("/projects/99999").to_request();
        let resp_not_found = test::call_service(&app, req_not_found).await;
        assert_eq!(resp_not_found.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_update_project_endpoint() {
        let (app, app_state) = setup_test_app().await;

        let project_am = openubl_entity::project::ActiveModel {
            name: sea_orm::Set("Original Name".to_string()),
            description: None,
            ..Default::default()
        };
        let created_project_ctx = app_state.system.persist_project(&project_am, openubl_api::db::Transactional::None).await.unwrap();
        let project_id = created_project_ctx.project.id;

        let update_dto = NewProjectDto {
            name: "Updated API Name".to_string(),
            description: Some("Updated API Desc".to_string()),
        };

        // Test update existing
        let req_update = test::TestRequest::put()
            .uri(&format!("/projects/{}", project_id))
            .set_json(&update_dto)
            .to_request();
        let resp_update: ProjectDto = test::call_and_read_body_json(&app, req_update).await;
        assert_eq!(resp_update.id, project_id);
        assert_eq!(resp_update.name, "Updated API Name");
        assert_eq!(resp_update.description, Some("Updated API Desc".to_string()));
        
        // Test update non-existent
        let req_update_not_found = test::TestRequest::put()
            .uri("/projects/99999")
            .set_json(&update_dto)
            .to_request();
        let resp_update_not_found = test::call_service(&app, req_update_not_found).await;
        assert_eq!(resp_update_not_found.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_delete_project_endpoint() {
        let (app, app_state) = setup_test_app().await;

        let project_am = openubl_entity::project::ActiveModel {
            name: sea_orm::Set("To Delete API".to_string()),
            description: None,
            ..Default::default()
        };
        let created_project_ctx = app_state.system.persist_project(&project_am, openubl_api::db::Transactional::None).await.unwrap();
        let project_id = created_project_ctx.project.id;

        // Test delete existing
        let req_delete = test::TestRequest::delete()
            .uri(&format!("/projects/{}", project_id))
            .to_request();
        let resp_delete = test::call_service(&app, req_delete).await;
        assert_eq!(resp_delete.status(), StatusCode::NO_CONTENT);

        // Verify it's gone
        let req_get_deleted = test::TestRequest::get()
            .uri(&format!("/projects/{}", project_id))
            .to_request();
        let resp_get_deleted = test::call_service(&app, req_get_deleted).await;
        assert_eq!(resp_get_deleted.status(), StatusCode::NOT_FOUND);

        // Test delete non-existent
        let req_delete_not_found = test::TestRequest::delete().uri("/projects/99999").to_request();
        let resp_delete_not_found = test::call_service(&app, req_delete_not_found).await;
        assert_eq!(resp_delete_not_found.status(), StatusCode::NOT_FOUND);
    }
}

#[post("/projects")]
async fn create_project(
    state: web::Data<AppState>,
    json: web::Json<NewProjectDto>,
) -> impl Responder {
    let new_project_dto = json.into_inner();
    let active_model: project::ActiveModel = new_project_dto.into();

    match active_model.insert(&state.db).await {
        Ok(inserted_project) => HttpResponse::Created().json(ProjectDto::from(inserted_project)),
        Err(e) => {
            log::error!("Failed to create project: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/projects/{project_id}")]
async fn get_project(
    state: web::Data<AppState>,
    path: web::Path<i32>,
) -> impl Responder {
    let project_id = path.into_inner();
    match project::Entity::find_by_id(project_id).one(&state.db).await {
        Ok(Some(project_model)) => HttpResponse::Ok().json(ProjectDto::from(project_model)),
        Ok(None) => HttpResponse::NotFound().body(format!("Project with ID {} not found", project_id)),
        Err(e) => {
            log::error!("Failed to get project {}: {:?}", project_id, e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[put("/projects/{project_id}")]
async fn update_project(
    state: web::Data<AppState>,
    path: web::Path<i32>,
    json: web::Json<NewProjectDto>,
) -> impl Responder {
    let project_id = path.into_inner();
    let updated_project_dto = json.into_inner();

    match project::Entity::find_by_id(project_id).one(&state.db).await {
        Ok(Some(project_model)) => {
            let mut active_model = project_model.into_active_model();
            active_model.name = Set(updated_project_dto.name);
            active_model.description = Set(updated_project_dto.description);
            match active_model.update(&state.db).await {
                Ok(updated_project) => HttpResponse::Ok().json(ProjectDto::from(updated_project)),
                Err(e) => {
                    log::error!("Failed to update project {}: {:?}", project_id, e);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        Ok(None) => HttpResponse::NotFound().body(format!("Project with ID {} not found to update", project_id)),
        Err(e) => {
            log::error!("Failed to find project {} for update: {:?}", project_id, e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[delete("/projects/{project_id}")]
async fn delete_project(
    state: web::Data<AppState>,
    path: web::Path<i32>,
) -> impl Responder {
    let project_id = path.into_inner();
    match project::Entity::delete_by_id(project_id).exec(&state.db).await {
        Ok(delete_result) => {
            if delete_result.rows_affected == 1 {
                HttpResponse::NoContent().finish()
            } else {
                HttpResponse::NotFound().body(format!("Project with ID {} not found to delete", project_id))
            }
        }
        Err(e) => {
            log::error!("Failed to delete project {}: {:?}", project_id, e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
