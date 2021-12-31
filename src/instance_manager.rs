use std::{fs, fs::File};
use std::collections::HashMap;
use std::io::prelude::*;
use mongodb::{bson::doc, options::ClientOptions, sync::Client};
use rocket::State;
use rocket::fairing::Result;
use serde_json::to_string;
use crate::MyManagedState;
use crate::instance::*;
use crate::util;
use uuid::Uuid; 

pub struct InstanceManager{
    instance_collection : HashMap<String, ServerInstance>,
    avail_ports : Vec<u16>,
    path : String, // must end with /
    mongodb : Client,
}


// TODO: DB IO
// TODO : should prob change parameter String to &str
impl InstanceManager {
    pub fn new(path : String, mongodb : Client) -> InstanceManager {
        InstanceManager{
            instance_collection : HashMap::new(),
            path,
            mongodb,
            avail_ports : vec![]
        }
    }
    // TODO: server.properties 
    pub async fn create_instance(&mut self, mut config : InstanceConfig, state: &State<MyManagedState>) -> Result<String, String> {
        config.name = sanitize_filename::sanitize(config.name);

        if self.check_if_name_exists(&config.name) {
            return Err(format!("{} already exists as an instance", &config.name));
        }
        let uuid = format!("{}", Uuid::new_v4());
        config.uuid = Some(uuid.clone());

        let path_to_instance = format!("{}{}/", self.path, config.name);

        fs::create_dir(path_to_instance.as_str()).map_err(|e| e.to_string())?;
        let instance = ServerInstance::new(&config, path_to_instance.clone());
        util::download_file(&config.url, format!("{}server.jar", &path_to_instance).as_str(), state, instance.uuid.as_str()).await?; // TODO: get rid of await

        let path_to_eula = format!("{}eula.txt", path_to_instance);
        let mut eula_file = File::create(path_to_eula.as_str()).map_err(|_|"failed to create eula.txt".to_string())?;
        eula_file.write_all(b"#generated by Lodestone\neula=true\n").map_err(|_| "failed to write to eula,txt".to_string())?;
        self.instance_collection.insert(uuid.clone(), instance);

        // TODO: DB IO
        /* TODO: 
            create a database with the uuid name 
            create config collection 
                config is everything needed to reconstruct the config 
                store InstanceConfig into database
        */ 
        let mongodb_client = self.mongodb.clone();
        mongodb_client
            .database(&uuid)
            .collection("config")
            .insert_one(doc! {
                "name": &config.name,
                "version": &config.version,
                "flavour": &config.flavour,
                "url": &config.url,
                "uuid": &config.uuid.unwrap(),
                "min_ram": &config.min_ram.unwrap(),
                "max_ram": &config.max_ram.unwrap()
            }, None).unwrap();

        Ok(uuid)
    }


    // TODO: basically drop database
    pub fn delete_instance(&mut self, uuid : String) -> Result<(), String> {
        match self.instance_collection.remove(&uuid) {
            None => Err("instance not found".to_string()),
            Some(instance) => {
                // handling db
                let mongodb_client = self.mongodb.clone();
                mongodb_client
                    .database(&uuid)
                    .drop(None)
                    .unwrap();
                
                    fs::remove_dir_all(format!("{}{}", self.path, instance.name)).map_err(|_| format!("{}{}", self.path, instance.name))?;
                Ok(())
            }
        }
    }

    pub fn clone_instance(&mut self, uuid : String) -> Result<(), String> {
        for pair in &self.instance_collection {
            if pair.0 == &uuid {
                if self.check_if_name_exists(&format!("{}_copy", &pair.1.name)) {
                    return Err(format!("{}_copy already exists as an instance", &pair.1.name));
                }
            }
        };
        Ok(())
    }

    
    pub fn send_command(&self, uuid : String, command : String) -> Result<(), String> {
        let instance = self.instance_collection.get(&uuid).ok_or("cannot send command to instance as it does not exist".to_string())?;
        instance.stdin.clone().unwrap().send(format!("{}\n", command)).map_err(|_| "failed to send command to instance".to_string())?;
        Ok(())
    }

    pub fn start_instance(&mut self, uuid : String) -> Result<(), String> {
        let instance = self.instance_collection.get_mut(&uuid).ok_or("instance cannot be started as it does not exist".to_string())?;
        instance.start(self.mongodb.clone())
    }

    pub fn stop_instance(&mut self, uuid : String) -> Result<(), String> {
        let instance = self.instance_collection.get_mut(&uuid).ok_or("instance cannot be stopped as it does not exist".to_string())?;
        instance.stop()
    }

    fn check_if_name_exists(&self, name : &String) -> bool {
        // TODO: DB IO
        let mut ret = false;
        for pair in &self.instance_collection {
            if &pair.1.name == name {
                ret = true;
                break; 
            }
        }
        ret
    }



}
