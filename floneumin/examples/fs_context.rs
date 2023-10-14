use floneumin_language::*;
use std::io::Write;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let documents = DocumentFolder::try_from(PathBuf::from("./documents")).unwrap();

    let mut database = DocumentDatabase::new(
        Bert::builder().build().unwrap(),
        ChunkStrategy::Sentence {
            sentence_count: 1,
            overlap: 0,
        },
    );
    database.extend(documents.clone()).await.unwrap();
    let mut fuzzy = FuzzySearchIndex::default();
    fuzzy.extend(documents).await.unwrap();

    loop {
        print!("Query: ");
        std::io::stdout().flush().unwrap();
        let mut user_question = String::new();
        std::io::stdin().read_line(&mut user_question).unwrap();

        println!(
            "vector: {:?}",
            database
                .search(&user_question, 5)
                .await
                .iter()
                .collect::<Vec<_>>()
        );
        println!(
            "fuzzy: {:?}",
            fuzzy
                .search(&user_question, 5)
                .await
                .iter()
                .collect::<Vec<_>>()
        );
    }
}
