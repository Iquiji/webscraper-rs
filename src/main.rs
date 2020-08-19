use crossbeam_channel::bounded;
use postgres::NoTls;
use r2d2_postgres::PostgresConnectionManager;
use select::document::Document;
use select::predicate::{Name, Predicate};
use std::env;
use std::sync::{atomic::AtomicU64, Arc};
use std::{collections::BTreeSet, thread, error::Error};
use url::Url;

static DURATION: u64 = 5000;
// TODO: async :/
fn main(){
    let (tx, rx) = bounded(100);
    let scraped_count: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    let scraped_last_duration = Arc::new(AtomicU64::new(0));
    let error_count = Arc::new(AtomicU64::new(0));

    let manager = PostgresConnectionManager::new(
        "host=free-db.coy5e9jykzwm.eu-central-1.rds.amazonaws.com user=postgres port=5432 password=Rd7rko$g85GV^&%123".parse().unwrap(),
        NoTls,
    );
    let pool = r2d2::Pool::new(manager).unwrap();

    let args: Vec<String> = env::args().collect();
    if args.contains(&"-c".to_owned()) {
        println!("only computing Weights");
        compute_rank(pool.clone().get().unwrap(), true);
        return;
    }
    let n_threads: usize = args[1]
        .parse()
        .expect("No Valid number for n_threads in ARGS");

    tx.send(Url::parse("http://leonroth.de/").unwrap()).unwrap();

    for _ in 0..n_threads {
        let thread_rx = rx.clone();
        let pool = pool.clone();
        let scraped_count = scraped_count.clone();
        let scraped_last_duration = scraped_last_duration.clone();
        let error_count = error_count.clone();

        thread::spawn(move || {
            // let mut db_client_try = pool.try_get();
            // while db_client_try.is_none(){
            //     println!("Error: failed to get db_client waiting 1000ms before retry");
            //     thread::sleep(std::time::Duration::from_millis(1000));
            //     db_client_try = pool.try_get();
            // }

            // let mut db_client = db_client_try.unwrap();
            for url in thread_rx {
                let db_try = pool.get();
                let db_client = match db_try {
                    Err(err) => {
                        eprintln!("{}", err);
                        continue;
                    }
                    Ok(ok) => ok,
                };

                match scrape_url(url.clone(),db_client) {
                    Ok(_) => {
                        scraped_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        scraped_last_duration.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                    Err(err) => {
                        let db_try = pool.get();
                        let mut db_client = match db_try {
                            Err(err) => {
                                eprintln!("{}", err);
                                continue;
                            }
                            Ok(ok) => ok,
                        };

                        eprintln!("failed to scrape with error: {}",err);
                        db_client.execute("UPDATE crawl_queue_v2 SET status = 'queued' WHERE url = $1",&[&url.as_str()]).unwrap();
                        error_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                };
                
            }
        });
    }
    println!(
        "Started Up {} new Threads for handling the channel",
        n_threads
    );
    // getting new Urls from Table 'crawl_queue_v2' :]
    let inputer_pool = pool.clone();
    thread::spawn(move || {
        let mut db_client = inputer_pool.clone().get().unwrap();
        let thread_tx = tx.clone();
        println!("Send Thread Starting Up");
        loop {
            let db_res = db_client.query("UPDATE crawl_queue_v2 SET status = 'processing' WHERE url = ANY (SELECT url FROM crawl_queue_v2 WHERE status = 'queued' ORDER BY timestamp ASC NULLS FIRST LIMIT 50) RETURNING * ;",&[]);
            match db_res {
                Ok(db_ok) => {
                    // println!("{:?}",db_ok)
                    for row in db_ok {
                        thread_tx.send(Url::parse(row.get(0)).unwrap()).unwrap();
                    }
                }
                Err(err) => {
                    println!("{}", err);
                    continue;
                }
            }
            //println!("getting new for sending!");
        }
    });
    // weight updater thread:
    thread::spawn(move || loop {
        thread::sleep(std::time::Duration::from_secs(120));
        let db_try = pool.get();
        let db_client = match db_try {
            Err(err) => {
                eprintln!("{}", err);
                continue;
            }
            Ok(ok) => ok,
        };
        compute_rank(db_client, true);
    });
    // printing crawl speed and what was crawled :]
    loop {
        thread::sleep(std::time::Duration::from_millis(DURATION));
        let last_duration: u64 = scraped_last_duration.swap(0, std::sync::atomic::Ordering::SeqCst);
        println!(
            "Scraped total: {}, Scraped per Minute: {}, Scraped last duration: {}, Error count: {}",
            scraped_count.load(std::sync::atomic::Ordering::SeqCst),
            last_duration * (60000 / DURATION),
            last_duration,
            error_count.load(std::sync::atomic::Ordering::Relaxed)
        );
    }
    // Ok(()) - not needed because of infinite loop above :)
}

fn scrape_url(url: url::Url,mut db_client: r2d2::PooledConnection<
    r2d2_postgres::PostgresConnectionManager<tokio_postgres::tls::NoTls>,
>) -> Result<(),Box<dyn Error>>{
    let hostname = Url::parse(&(url.scheme().to_owned() + "://" + url.host_str().unwrap() + "/"))?;
    // let mut db_client = pool.get().unwrap(); // Why does this fail sometimes ?!

    //println!("Scraping: {}",&url);

    let body = reqwest::blocking::get(url.as_str())?;

    let document = Document::from_read(std::io::Read::take(body, 1048576))?;

    let new_urls: BTreeSet<_> = document
        .find(Name("a"))
        .filter_map(|n| n.attr("href"))
        .map(|x| {
            let res_url = if x.starts_with("http") {
                Url::parse(x).unwrap()
            } else {
                url.join(x).unwrap()
            };
            let res_url_query = res_url.query();
            if res_url_query.is_some() {
                Url::parse(
                    &res_url
                        .as_str()
                        .to_owned()
                        .replace(res_url.query().unwrap(), ""),
                )
                .unwrap()
            } else {
                res_url
            }
        })
        .collect();

    let text: Vec<String> = document
        .find(
            Name("p")
                .or(Name("h1"))
                .or(Name("h2"))
                .or(Name("h3"))
                .or(Name("h4"))
                .or(Name("h5"))
                .or(Name("title")),
        )
        .map(|f| {
            f.text()
                .replace("\n", "")
                .replace("\t", "")
                .replace("     ", "")
                .replace("<b>", "")
                .replace("</b>", "")
        })
        .collect();
    // see file:///Users/leon/.rustup/toolchains/stable-x86_64-apple-darwin/share/doc/rust/html/std/primitive.slice.html#method.join
    let mut text_string = "".to_string();
    for string in text.clone() {
        text_string.push_str(" ");
        text_string.push_str(&string);
    }
    // ADD to websites_v2
    db_client.query("WITH before AS (SELECT * FROM websites_v2 WHERE url = $1 ), inserted AS (INSERT INTO websites_v2 (url,text,last_scraped,text_tsvector,hostname) VALUES ($1,$2,NOW(),to_tsvector('english',$2),$3) ON CONFLICT (url) DO UPDATE SET last_scraped = NOW(), text = $2 , text_tsvector = to_tsvector('english',$2) WHERE websites_v2.url = $1) SELECT last_scraped FROM before;",&[&url.as_str(),&text_string.get(..(text_string.chars().map(|_| 1).sum::<usize>()).max(10000)).to_owned(),&hostname.as_str()])?;

    // make target base urls
    let target_base_urls: BTreeSet<Url> = new_urls
        .clone()
        .into_iter()
        .map(|target_url| {
            Url::parse(
                &(target_url.scheme().to_owned()
                    + "://"
                    + target_url.host_str().unwrap_or_else(|| "example.com")
                    + "/"),
            )
            .unwrap()
        })
        .collect();

    // insert into crawl_queue_v2 with timestamp NULL
    for new_url in new_urls {
        db_client.query("INSERT INTO crawl_queue_v2 (url,timestamp,status) VALUES ($1,NULL,$2) ON CONFLICT (url) DO NOTHING",&[&new_url.as_str(),&"queued"])?;
    }

    // insert into base_url_links
    for target_url in target_base_urls {
        if target_url.host_str().unwrap() == url.host_str().unwrap() {
            continue;
        } else if target_url.host_str().unwrap() == "example.com" {
            eprintln!("Failed to parse host url because it contains 'example.com'");
            continue;
        }
        //println!("{}",target_url);
        let db_res = db_client.query("WITH inserted AS ( INSERT INTO base_url_links (base_url,target_url) VALUES ($1,$2) ON CONFLICT (base_url,target_url) DO NOTHING RETURNING target_url) UPDATE websites_v2 SET popularity = websites_v2.popularity + 1 FROM inserted WHERE hostname = inserted.target_url;",
            &[&Url::parse(&(url.scheme().to_owned() + "://" + url.host_str().unwrap() + "/")).unwrap().as_str(),&target_url.as_str()]);
        match db_res {
            Ok(_) => {
                //println!("{:?}",db_ok);
            }
            Err(err) => {
                println!("{}", err);
                continue;
            }
        }
    }

    // Update crawl_queue_v2 to say finishged crawling back in queue with timestamp now
    db_client.execute("UPDATE crawl_queue_v2 SET status = 'queued' , timestamp = current_timestamp WHERE url = $1",&[&url.as_str()])?;

    Ok(())
}

fn compute_rank(
    mut db_client: r2d2::PooledConnection<
        r2d2_postgres::PostgresConnectionManager<tokio_postgres::tls::NoTls>,
    >,
    on_db: bool,
) {
    if on_db {
        // TO INSERT query!
        db_client.execute(r#"WITH target_info AS(SELECT target_url,count(1)AS count,SUM(weight)AS total_weight FROM base_url_links GROUP BY target_url),updated_links AS(UPDATE base_url_links SET weight=(target_info.total_weight/count)*0.85 FROM target_info WHERE base_url=target_info.target_url)UPDATE websites_v2 SET"rank"=target_info.total_weight FROM target_info WHERE hostname = target_info.target_url;"#,&[]).unwrap();
        return;
    }
    println!("Computing new Weights/Ranks...");
    let now = std::time::Instant::now();
    let mut total_updated = 0;

    let mut websites: BTreeSet<String> = BTreeSet::new();
    match db_client.query(
        "SELECT * FROM websites_v2 ORDER BY last_scraped DESC LIMIT 1000",
        &[],
    ) {
        Ok(val) => {
            for row in val {
                let url: String = row.get(0);
                websites.insert(url);
            }
        }
        Err(err) => {
            println!("{}", err);
            panic!();
        }
    };
    websites.into_iter().for_each(|url | {
        let mut all_weights : Vec<f64> = vec![];
        let mut all_weights_with_linkage : Vec<(String,String,f64)> = vec![];
        let url_as_url =Url::parse(&url).unwrap();
        let mut base_url : String = "".to_owned();

        // get all links where target_url == our website

        match db_client.query("SELECT weight,base_url,target_url FROM base_url_links WHERE target_url LIKE '%' || $1 ||'%';",&[&Url::parse(&(url_as_url.scheme().to_owned() + "://" + url_as_url.host_str().unwrap() + "/")).unwrap().as_str()]){
            Ok(val) => {
                for row in val {
                    let weight : f64 = row.get(0);
                    base_url = row.get(1);
                    let target_url : String = row.get(2);

                    all_weights.push(weight);

                    all_weights_with_linkage.push((base_url.clone(),target_url,weight));
                }
            }
            Err(err) => {
                panic!(err);
            }
        };

        // make weight of our website
        let total_weight = all_weights.into_iter().sum::<f64>() * 0.85;

        // get how many links to from website exist
        let how_many_links = db_client.query("SELECT COUNT(*) FROM base_url_links WHERE base_url = $1",&[&base_url]).unwrap();
        let mut linkage_count : i64 = 0;

        for row in how_many_links{
            linkage_count = row.get(0);
        }

        // get weight per link by total_weight/linkage count
        let weight_per_link : f64;
        if linkage_count != 0 {
            weight_per_link = total_weight / linkage_count as f64;
        }else{
            weight_per_link = 0.05
        }
        // set weight of all links where base url == our website to weight per link
        db_client.query("UPDATE base_url_links SET weight = $2 WHERE base_url = $1",&[&base_url,&weight_per_link]).unwrap(); 
        // set rank/weight of our website
        db_client.execute("UPDATE websites_v2 SET rank = $2 WHERE url = $1",&[&url,&total_weight]).unwrap();
        //println!("Setting url: {} to weight: {}, total of {} links get weight: {}",&url,total_weight,linkage_count,weight_per_link);
        total_updated += 1;
    });
    println!(
        "Updated {} Websites in {} seconds",
        total_updated,
        now.elapsed().as_secs()
    );
}