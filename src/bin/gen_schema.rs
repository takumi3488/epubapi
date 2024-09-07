use epubapi::routes::ApiDoc;
use utoipa::OpenApi;

fn main() {
    let res = ApiDoc::openapi().to_pretty_json().unwrap();
    println!("{}", res);
    let out = res
        .split("\n")
        .enumerate()
        .filter(|&(_, x)| x.contains("$ref"))
        .map(|(i, x)| format!("{}: {}", i + 1, x))
        .collect::<Vec<_>>()
        .join("\n");
    if !out.is_empty() {
        panic!("{}", out);
    }
}
