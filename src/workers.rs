use crate::state::Store;

pub fn print_workers(store: &Store) -> crate::Result<()> {
    for worker in store.workers()? {
        println!(
            "{}\t{}\t{}\t{}",
            worker.id,
            worker.status,
            worker.endpoint,
            worker.capabilities.join(",")
        );
    }
    Ok(())
}
