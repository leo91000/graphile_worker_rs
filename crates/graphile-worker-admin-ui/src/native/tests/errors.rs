use super::*;

#[test]
fn admin_query_not_found_maps_to_not_found() {
    let error = ApiError::from(AdminQueryError::NotFound("job 42 not found".to_string()));

    assert_eq!(error.status, StatusCode::NOT_FOUND);
    assert!(error.message.contains("42"));
}

#[test]
fn sqlx_row_not_found_maps_to_not_found() {
    let error = ApiError::from(sqlx::Error::RowNotFound);

    assert_eq!(error.status, StatusCode::NOT_FOUND);
    assert_eq!(error.message, "resource not found");
}

#[test]
fn internal_errors_hide_error_details_from_clients() {
    let error = ApiError::internal("database password leaked");

    assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(error.message, "internal server error");
}
