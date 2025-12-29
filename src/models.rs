use diesel::prelude::*;
use crate::schema::*;
use chrono::NaiveDate;

#[derive(Queryable, Insertable, Identifiable, Clone, Debug)]
#[diesel(table_name = categories)]
pub struct Category {
    pub id: Vec<u8>,
    pub name: String,
}

#[derive(Queryable, Insertable, Identifiable, Associations, Clone, Debug)]
#[diesel(belongs_to(Category))]
#[diesel(table_name = expenses)]
pub struct Expense {
    pub id: Vec<u8>,
    pub category_id: Vec<u8>,
    pub amount: f64,
    pub comment: Option<String>,
    pub date: NaiveDate,
}

#[derive(Queryable, Insertable, Identifiable, Clone, Debug)]
#[diesel(table_name = top_up_categories)]
pub struct TopUpCategory {
    pub id: Vec<u8>,
    pub name: String,
}

#[derive(Queryable, Insertable, Identifiable, Associations, Clone, Debug)]
#[diesel(belongs_to(TopUpCategory, foreign_key = category_id))]
#[diesel(table_name = top_ups)]
pub struct TopUp {
    pub id: Vec<u8>,
    pub category_id: Vec<u8>,
    pub amount: f64,
    pub comment: Option<String>,
    pub date: NaiveDate,
}


