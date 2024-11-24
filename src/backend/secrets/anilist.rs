#[cfg(test)]
mod tests {
    use clap::crate_name;
    use uuid::Uuid;

    use super::*;

    #[derive(Debug)]
    struct AnilistStorage;

    //#[test]
    //fn it_stores_anilist_account_secrets() {
    //    let id = Uuid::new_v4().to_string();
    //    let code = Uuid::new_v4().to_string();
    //    let secret = Uuid::new_v4().to_string();
    //
    //    AnilistStorage::store(id, code, secret);
    //
    //    let (id_stored, code_stored, secret_stored) = AnilistStorage::get_credentials();
    //
    //    assert_eq!(id, id_stored);
    //    assert_eq!(code, code_stored);
    //    assert_eq!(secret, secret_stored);
    //}
}
