// @generated automatically by Diesel CLI.

diesel::table! {
    categories (id) {
        id -> Binary,
        name -> Text,
    }
}

diesel::table! {
    expenses (id) {
        id -> Binary,
        category_id -> Binary,
        amount -> Float,
        comment -> Nullable<Text>,
        date -> Text,
    }
}

diesel::table! {
    top_up_categories (id) {
        id -> Binary,
        name -> Text,
    }
}

diesel::table! {
    top_ups (id) {
        id -> Binary,
        category_id -> Binary,
        amount -> Double,
        comment -> Nullable<Text>,
        date -> Date,
    }
}

diesel::allow_tables_to_appear_in_same_query!(categories, expenses, top_up_categories, top_ups,);
