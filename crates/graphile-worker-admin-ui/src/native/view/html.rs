pub(super) fn render_document(body: String) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="color-scheme" content="light dark">
  <title>Graphile Worker Admin</title>
  <link rel="icon" href="/favicon.ico" type="image/svg+xml">
  <link rel="stylesheet" href="/assets/admin.css">
</head>
<body>
{body}
<script type="module" src="/assets/admin.js"></script>
</body>
</html>"#
    )
}
