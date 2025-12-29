use diesel::prelude::*;
use crate::models::Category;
use crate::schema;
use uuid::Uuid;

impl Category {
    pub fn create(conn: &mut SqliteConnection, name: &str) -> QueryResult<Category> {
        use schema::categories;
        let id = Uuid::new_v4().as_bytes().to_vec();
        let new_category = (
            categories::id.eq(&id),
            categories::name.eq(name),
        );
        diesel::insert_into(categories::table)
            .values(new_category)
            .execute(conn)?;
        categories::table.filter(categories::id.eq(&id)).first(conn)
    }

    pub fn read_all(conn: &mut SqliteConnection) -> QueryResult<Vec<Category>> {
        use schema::categories::dsl::*;
        categories.load::<Category>(conn)
    }

    pub fn update(conn: &mut SqliteConnection, category_id: &[u8], new_name: &str) -> QueryResult<usize> {
        use schema::categories::dsl::*;
        diesel::update(categories.filter(id.eq(category_id)))
            .set(name.eq(new_name))
            .execute(conn)
    }

    pub fn delete(conn: &mut SqliteConnection, category_id: &[u8]) -> QueryResult<usize> {
        use schema::categories::dsl::*;
        diesel::delete(categories.filter(id.eq(category_id))).execute(conn)
    }
}

