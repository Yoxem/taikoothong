#![feature(proc_macro_hygiene, decl_macro)]


use chrono::{TimeZone, Utc, NaiveDateTime};
use curl::easy::Easy;
use serde::Serialize;
use serde_json::{Result, Value, Value::Array}; // JSON
use std::collections::HashMap;
use rocket_dyn_templates::Template;
use rocket::{Rocket, Build};
use round::round;
use num_format::{Locale, WriteFormatted};
use rss::Channel;
use feed_rs::parser;

//use std::env;
//use std::fs;

#[macro_use] extern crate rocket;

enum Date {
    Day(u8),
    Week(u8),
    Month(u8),
    Max,
    YearToDate}


    #[derive(Serialize, Debug)]
    struct TemplateContext<'a> {
        main : HashMap<&'a str, Vec<String>>,
    news: Vec<HashMap<&'a str, String>>,
    }

impl Date {
    fn as_str(&self) -> String {
        match &self {
            Date::Day(d) => format!("{:?}d", d),
            Date::Week(wk) => format!("{:?}wk", wk),
            Date::Month(mo) => format!("{:?}mo", mo),
            Date::YearToDate => format!("ytd"),
            Date::Max => format!("max"),
        }
    }
}


#[get("/<stock_id>/json")]
fn get_tw_stock_json(stock_id: String) -> String {
    let response_body = get_stock_data(stock_id.as_str(), Date::Day(1), Date::YearToDate);
    let response_json: Value = serde_json::from_str(response_body.as_str()).unwrap();


    let mut stock_main_data = tw_stock_process_json(&response_json);

    let stock_main_data_json = serde_json::json!(stock_main_data);

    return stock_main_data_json.to_string();
}


fn tw_stock_process_json(response_json : &Value) -> HashMap<&str, Vec<String>>{


    let days_in_unix_time = &response_json["chart"]["result"][0]["timestamp"];

    let mut stock_main_data = HashMap::new();

    let days_in_custom_format = match days_in_unix_time {
        serde_json::Value::Array(days_vec) => days_vec
            .iter()
            .map(|day|json_unix_time_to_date(day))
            .collect::<Vec<_>>(),
        _ => vec![format!("Not a series of date")],
    };

    stock_main_data.insert("date",  days_in_custom_format);

    let mut open_prices : Vec<String> = vec![];
    let mut close_prices : Vec<String> = vec![];
    let mut high_prices : Vec<String> = vec![];
    let mut low_prices : Vec<String> = vec![];
    let mut volumes : Vec<String> = vec![];


    let price_and_volume_infos = &response_json["chart"]["result"][0]["indicators"]["quote"][0];
    
    let open_prices_orig = &price_and_volume_infos["open"];
    let close_prices_orig = &price_and_volume_infos["close"];
    let high_prices_orig = &price_and_volume_infos["high"];
    let low_prices_orig = &price_and_volume_infos["low"];
    let volumes_orig = &price_and_volume_infos["volume"];


    match (open_prices_orig, close_prices_orig, high_prices_orig, low_prices_orig, volumes_orig){
        (Array(o), Array(c), Array(h),
            Array(l), Array(v)) => {
            for i in 0..(o.len()){
                open_prices.push(format!("{:0.2}", o[i].as_f64().unwrap()));
                close_prices.push(format!("{:0.2}", c[i].as_f64().unwrap()));
                high_prices.push(format!("{:0.2}", h[i].as_f64().unwrap()));
                low_prices.push(format!("{:0.2}", l[i].as_f64().unwrap()));
                let mut formatted_volume = String::new();
                formatted_volume.write_formatted(&v[i].as_i64().unwrap(), &Locale::zh);

                volumes.push(formatted_volume);
            }
            
        },
        _ => (),
    }

    stock_main_data.insert("open", open_prices);
    stock_main_data.insert("close", close_prices);
    stock_main_data.insert("high", high_prices);
    stock_main_data.insert("low", low_prices);
    stock_main_data.insert("volume", volumes);

    return stock_main_data; 
}

#[get("/<stock_id>")]
fn get_tw_stock(stock_id: String) -> Template {

   let rss_xml = get_rss_data(stock_id.as_str());

    //let rss_xml = fs::read_to_string("/tmp/a.rss")
    //    .expect("Should have been able to read the file");

    let rss_parsed = parser::parse(rss_xml.as_bytes()).unwrap();


    let response_body = get_stock_data(stock_id.as_str(), Date::Day(1), Date::YearToDate);
    let response_json: Value = serde_json::from_str(response_body.as_str()).unwrap();

    let mut stock_main_data = tw_stock_process_json(&response_json);
    stock_main_data.insert("stock_id", vec![stock_id]);


    let mut rss_entries = vec![];

    for i in 0..rss_parsed.entries.len(){
        let mut rss_entry = HashMap::new();
        //let title = i.title.clone().unwrap().content;
        //println!("{:}", title);
        let title = &rss_parsed.entries[i].title;

        let title_string = match title {
            Some(a) => a.clone().content,
            _ => "title reading error".to_string(),
        };
        rss_entry.insert("title", title_string);


        let time = &rss_parsed.entries[i].published;
        let date_string = match time {
            Some(a) => {

                 format!("{}", a.format("%Y-%m-%d"))},
            _ => "time reading error".to_string(),
        };
        rss_entry.insert("date", date_string);


        let link = &rss_parsed.entries[i].links[0].href;
        rss_entry.insert("link", link.to_string());

        let summary = &rss_parsed.entries[i].summary;
        let summary_string = match summary {
            Some(a) => a.clone().content,
            _ => "summary reading error".to_string(),
        };
        rss_entry.insert("summary", summary_string); 

        rss_entries.push(rss_entry);
    }

    println!("{:?}", rss_entries);
    


    let stock_total_data = TemplateContext{main : stock_main_data.clone(), news : rss_entries};

    let mut stock_main_data_by_date = transverse_stock_data_by_date(stock_main_data.clone());
    //let mut stock_main_data_by_date_wrapper = HashMap::new();

    //stock_main_data_by_date_wrapper.insert("data", stock_main_data_by_date);

    return Template::render("tw_stock", stock_total_data);
}

fn json_unix_time_to_date(json_value: &Value) -> String {
    let unix_time = json_value.as_i64().unwrap();

    let naive_time = Utc.timestamp_opt(unix_time, 0).unwrap();

    let date = format!("{}", naive_time.format("%Y-%m-%d"));

    return date;
}

fn transverse_stock_data_by_date(orig_data : HashMap<&str, Vec<String>>) -> 
    Vec<HashMap<String, String>>{
    let mut stock_data_by_date = vec![];
    let dates = &orig_data["date"];

    for i in 0..dates.len()-1{
        let mut day_hash_map = HashMap::new();
        day_hash_map.insert(format!("date"), orig_data["date"][i].clone());
        day_hash_map.insert(format!("open"), orig_data["open"][i].clone());
        day_hash_map.insert(format!("close"), orig_data["close"][i].clone());
        day_hash_map.insert(format!("high"), orig_data["high"][i].clone());
        day_hash_map.insert(format!("low"), orig_data["low"][i].clone());
        day_hash_map.insert(format!("volume"), orig_data["volume"][i].clone());

        stock_data_by_date.push(day_hash_map);

    }

    return stock_data_by_date;


}

fn get_rss_data(stock_id :  &str) -> String{
    let url = format!("https://tw.stock.yahoo.com/rss?s={:}",
        stock_id
    );



    return get_url_data(&url);
}

fn get_stock_data(stock_id: &str, interval: Date, range: Date) -> String {
    let intrval_str = interval.as_str();
    let range_str = range.as_str();

    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/\
        {:}.TW?metrics=Date,High,Low,Open,Close,Volume&interval={:}&range={:}",
        stock_id, intrval_str, range_str
    );



    return get_url_data(&url);
}

fn get_url_data(url : &String) -> String{
let mut curl_easy = Easy::new(); // fetch the data with the curl binding
let mut response = String::new();

{
    curl_easy.url(url.as_str()).unwrap();

    let mut curl_transfer = curl_easy.transfer();

    curl_transfer
        .write_function(|data| {
            let s = match std::str::from_utf8(data){
                Err(_) => {println!("解碼錯誤"); ""}
                Ok(cont) => { println!("解碼成功"); cont}
            };
            response.push_str(s);
            Ok(data.len())
        })
        .unwrap();

    curl_transfer.perform().unwrap();
}

return response.clone();
}


#[launch]
fn rocket() -> Rocket<Build>  {
    // rocket::ignite().mount("/", routes![index]).launch();
    rocket::build().attach(Template::fairing())
        .mount("/tw", routes![get_tw_stock, get_tw_stock_json])


}
