use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait, IntoActiveModel, PaginatorTrait, QueryOrder, Set};
use openubl_entity as entity;

use crate::db::{Paginated, PaginatedResults, Transactional};
use crate::system::error::Error;
use crate::system::InnerSystem;

#[derive(Debug, Clone)]
pub struct ProjectContext {
    pub system: InnerSystem,
    pub project: entity::project::Model,
}

impl From<(&InnerSystem, entity::project::Model)> for ProjectContext {
    fn from((system, project): (&InnerSystem, entity::project::Model)) -> Self {
        Self {
            system: system.clone(),
            project,
        }
    }
}

impl InnerSystem {
    pub async fn find_project_by_id(
        &self,
        id: i32,
        tx: Transactional<'_>,
    ) -> Result<Option<ProjectContext>, Error> {
        Ok(entity::project::Entity::find_by_id(id)
            .one(&self.connection(tx))
            .await?
            .map(|e| (self, e).into()))
    }

    pub async fn list_projects(
        &self,
        paginated: Paginated,
        tx: Transactional<'_>,
    ) -> Result<PaginatedResults<ProjectContext>, Error> {
        let connection = self.connection(tx);

        let query = entity::project::Entity::find()
            .order_by_asc(entity::project::Column::Name); // Or any other default sort

        let num_items = query.clone().count(&connection).await?;
        
        // Use methods from Paginated struct if they are correct, or direct field access
        let items_per_page = paginated.limit(); // Or paginated.limit directly
        let current_page_0_indexed = if items_per_page == 0 { 0 } else { paginated.offset() / items_per_page }; // Prevent division by zero, ensure offset() is available or use paginated.offset

        let projects = query
            .paginate(&connection, items_per_page)
            .fetch_page(current_page_0_indexed as usize) // fetch_page is 0-indexed
            .await?;

        let project_contexts = projects
            .into_iter()
            .map(|p| (self, p).into())
            .collect();

        Ok(PaginatedResults {
            items: project_contexts,
            num_items,
        })
    }

    pub async fn persist_project(
        &self,
        model: &entity::project::ActiveModel, // Changed from Model to ActiveModel
        tx: Transactional<'_>,
    ) -> Result<ProjectContext, Error> {
        // It's an ActiveModel already, so we can directly insert or update
        // Assuming 'id' is not set for new entities, or is handled by the db (auto_increment)
        // If 'id' is set, this might behave as an update if the PK exists, or insert if not.
        // For clarity, SeaORM typically uses `insert` for new and `update` for existing.
        // The `save` method can insert or update.
        let result: entity::project::Model = model.clone().save(&self.connection(tx)).await?;
        Ok((self, result).into())
    }
}

impl ProjectContext {
    pub async fn update(
        &mut self, // Changed to &mut self to update self.project
        model: &entity::project::ActiveModel,
        tx: Transactional<'_>,
    ) -> Result<(), Error> {
        let mut active_model: entity::project::ActiveModel = self.project.clone().into_active_model();

        // Only apply changes if they are Set. Caller should construct ActiveModel appropriately.
        if let ActiveValue::Set(name) = model.name {
            active_model.name = ActiveValue::Set(name);
        }
        if let ActiveValue::Set(description) = model.description.clone() { // Clone Option<String>
            active_model.description = ActiveValue::Set(description);
        }
        // ID should not be changed via this method.

        self.project = active_model.update(&self.system.connection(tx)).await?;
        Ok(())
    }

    pub async fn delete(self, tx: Transactional<'_>) -> Result<(), Error> { // Changed to consume self
        let active_model: entity::project::ActiveModel = self.project.into_active_model();
        active_model.delete(&self.system.connection(tx)).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Transactional;
    use openubl_entity::project;
    // Assuming Set is still needed for new_project_active_model, qualify it or ensure ActiveValue is imported for tests too.
    // For clarity, using sea_orm::ActiveValue::Set in new_project_active_model if tests also had issues.
    // The main code fix is `use sea_orm::ActiveValue;` and then using `ActiveValue::Set`.
    // The test code itself uses `sea_orm::ActiveValue::Set` already in its `new_project_active_model`.

    async fn create_system() -> InnerSystem {
        InnerSystem::for_test().await.unwrap().as_ref().clone()
    }

    fn new_project_active_model(name: &str, description: Option<&str>) -> project::ActiveModel {
        project::ActiveModel {
            name: sea_orm::ActiveValue::Set(name.to_string()),
            description: sea_orm::ActiveValue::Set(description.map(|s| s.to_string())),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_persist_and_find_project() {
        let system = create_system().await;
        let project_am = new_project_active_model("Test Project 1", Some("Description 1"));

        // Persist
        let project_ctx = system
            .persist_project(&project_am, Transactional::None)
            .await
            .unwrap();
        assert_eq!(project_ctx.project.name, "Test Project 1");
        assert_eq!(project_ctx.project.description, Some("Description 1".to_string()));
        let project_id = project_ctx.project.id;

        // Find
        let found_ctx = system
            .find_project_by_id(project_id, Transactional::None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found_ctx.project.id, project_id);
        assert_eq!(found_ctx.project.name, "Test Project 1");

        // Find non-existent
        let not_found_ctx = system
            .find_project_by_id(9999, Transactional::None)
            .await
            .unwrap();
        assert!(not_found_ctx.is_none());
    }

    #[tokio::test]
    async fn test_list_projects() {
        let system = create_system().await;

        // Create some projects
        let project1_am = new_project_active_model("Project A", None);
        system.persist_project(&project1_am, Transactional::None).await.unwrap();

        let project2_am = new_project_active_model("Project B", Some("Desc B"));
        system.persist_project(&project2_am, Transactional::None).await.unwrap();
        
        let project3_am = new_project_active_model("Project C", None);
        system.persist_project(&project3_am, Transactional::None).await.unwrap();

        // List first page
        let paginated_results_p1 = system
            .list_projects(Paginated { offset: 0, limit: 2 }, Transactional::None)
            .await
            .unwrap();
        
        assert_eq!(paginated_results_p1.num_items, 3);
        assert_eq!(paginated_results_p1.items.len(), 2);
        assert_eq!(paginated_results_p1.items[0].project.name, "Project A"); // Assuming order by name ASC
        assert_eq!(paginated_results_p1.items[1].project.name, "Project B");

        // List second page
        let paginated_results_p2 = system
            .list_projects(Paginated { offset: 2, limit: 2 }, Transactional::None)
            .await
            .unwrap();

        assert_eq!(paginated_results_p2.num_items, 3);
        assert_eq!(paginated_results_p2.items.len(), 1);
        assert_eq!(paginated_results_p2.items[0].project.name, "Project C");

        // List with page size larger than total items
        let paginated_results_all = system
            .list_projects(Paginated { offset: 0, limit: 5 }, Transactional::None)
            .await
            .unwrap();
        assert_eq!(paginated_results_all.items.len(), 3);
    }

    #[tokio::test]
    async fn test_project_context_update() {
        let system = create_system().await;
        let project_am = new_project_active_model("Initial Name", Some("Initial Desc"));
        
        let mut project_ctx = system.persist_project(&project_am, Transactional::None).await.unwrap();
        let project_id = project_ctx.project.id;

        let update_am = project::ActiveModel {
            name: sea_orm::ActiveValue::Set("Updated Name".to_string()),
            description: sea_orm::ActiveValue::Set(Some("Updated Desc".to_string())),
            ..Default::default() // ID is not set here for update
        };

        project_ctx.update(&update_am, Transactional::None).await.unwrap();
        
        // Verify internal context model updated
        assert_eq!(project_ctx.project.name, "Updated Name");
        assert_eq!(project_ctx.project.description, Some("Updated Desc".to_string()));

        // Verify by fetching again
        let found_ctx = system.find_project_by_id(project_id, Transactional::None).await.unwrap().unwrap();
        assert_eq!(found_ctx.project.name, "Updated Name");
        assert_eq!(found_ctx.project.description, Some("Updated Desc".to_string()));
    }

    #[tokio::test]
    async fn test_project_context_delete() {
        let system = create_system().await;
        let project_am = new_project_active_model("To Be Deleted", None);
        let project_ctx = system.persist_project(&project_am, Transactional::None).await.unwrap();
        let project_id = project_ctx.project.id;

        project_ctx.delete(Transactional::None).await.unwrap();

        let found_ctx = system.find_project_by_id(project_id, Transactional::None).await.unwrap();
        assert!(found_ctx.is_none(), "Project should be deleted");
    }
}
