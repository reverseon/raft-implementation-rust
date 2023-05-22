use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>
{
    let vec = Arc::new(Mutex::new(Vec::new()));
    let mut handles = vec![];

    for i in 1..=10 {
        let vec_clone = Arc::clone(&vec);
        let handle = tokio::spawn(async move {
            let mut vec = vec_clone.lock().expect("Failed to acquire lock");
            vec.push(i);
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.await?;
    }

    let locked_vec = vec.lock().expect("Failed to acquire lock");
    println!("{:?}", *locked_vec);
   Ok(())

}