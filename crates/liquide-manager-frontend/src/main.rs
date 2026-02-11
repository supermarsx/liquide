use liquide_manager_frontend::FrontendRuntime;

fn main() {
    let rt = FrontendRuntime::new();
    println!(
        "LiquiDE Manager Frontend — {} pages, theme: {}",
        rt.page_count(),
        rt.theme().name
    );
}
