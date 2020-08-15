use crossbeam_channel::bounded;
use std::thread;
use std::sync::{Arc,Mutex};
use postgres::{Client,NoTls};
// use native_tls::{Certificate, TlsConnector};
// use postgres_native_tls::MakeTlsConnector;
// use std::fs;


fn main() {
    let (tx,rx) = bounded(500);

    // let cert = fs::read("./postgres-mac.pem").unwrap();
    // let cert = Certificate::from_pem(&cert).unwrap();
    // let connector = TlsConnector::builder()
    //     .add_root_certificate(cert)
    //     .build().unwrap();
    // let connector = MakeTlsConnector::new(connector);

    let db_client = postgres::Client::connect(
        "host=free-db.coy5e9jykzwm.eu-central-1.rds.amazonaws.com user=postgres port=5432 password=Rd7rko$g85GV^&%123",
        NoTls,
    ).unwrap();

    for i in 0..5{
        let thread_tx = tx.clone();
        let thread_rx = rx.clone();

        thread::spawn(move || {
            loop{
                let url : String = thread_rx.recv().unwrap();
                let mut backfeed_num : usize = 0;
                let new_urls: Vec<String> = vec![];
    
                // do something
                
                for new_url in new_urls{
                    if backfeed_num >= 4 || thread_tx.is_full(){
                        continue;
                    }else{
                        thread_tx.send("unimplemented".to_string());
                        backfeed_num += 1;
                    }
                }
            }

        });
    }
}