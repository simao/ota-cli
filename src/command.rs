use std::collections::HashMap;
use std::io::{self, Read};
use std::str::FromStr;

use clap::ArgMatches;
use reqwest::blocking::Response;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::api::{
    campaigner::{Campaigner, CampaignerApi},
    director::{Director, DirectorApi, TargetRequests, TufUpdates},
    registry::{DeviceType, GroupType, Registry, RegistryApi},
    reposerver::{Reposerver, ReposerverApi, TargetPackages, TufPackage, TufPackages},
};
use crate::config::Config;
use crate::error::{Error, Result};



/// Execute a command then handle the HTTP `Response`.
pub trait Exec<'a> {
    fn exec(&self, args: &ArgMatches<'a>) -> Result<CommandResult>;
}

/// Available CLI sub-commands.
#[derive(Serialize, Deserialize, PartialEq, Clone, Copy, Debug)]
pub enum Command {
    Init,
    Campaign,
    Device,
    Group,
    Package,
    Update,
}

pub enum CommandResult {
    Table(TableResult),
    Http(Response),
    Empty
}

impl CommandResult {
    fn headers(self: &Self) -> HashMap<String, String> {
        let empty = HeaderMap::new();

        let headers = match self {
            CommandResult::Table(r) => &r.headers,
            CommandResult::Http(r) => &r.headers(),
            CommandResult::Empty => &empty,
        };

        let mut res: HashMap<String, String> = HashMap::new();

        for (k, v) in headers.iter() {
            res.insert(k.to_string(), v.to_str().unwrap_or("").to_owned());
        }

        res
    }
}

impl From<Response> for CommandResult {
    fn from(r: Response) -> Self {
        CommandResult::Http(r)
    }
}

impl From<TableResult> for CommandResult {
    fn from(r: TableResult) -> Self {
        CommandResult::Table(r)
    }
}

pub struct TableResult {
    pub headers: HeaderMap,
    pub table: comfy_table::Table,
    pub response: Vec<u8>
}

impl TableResult {
    pub fn new(headers: HeaderMap, response: Vec<u8>, table: comfy_table::Table) -> TableResult {
        TableResult { headers, table, response }
    }
}

pub fn print_command_result(use_tables: bool, resp: CommandResult) -> Result<()> {
    debug!("response headers:\n{:#?}", resp.headers());

    match resp {
        CommandResult::Table(r) if use_tables => {
            io::copy(&mut r.table.to_string().as_bytes(), &mut io::stdout())?;
            ()
        },

        CommandResult::Table(r) => {
            print_http_response(&mut r.response.as_slice())?;
            ()
        },

        CommandResult::Http(mut r) => {
            print_http_response(&mut r)?;
            ()
        },

        CommandResult::Empty =>
            ()
    }

    Ok(())
}

fn print_http_response(resp: &mut dyn Read) -> Result<()> {
    let mut body = Vec::new();
    debug!("response length: {}\n", resp.read_to_end(&mut body)?);

    let out = if let Ok(json) = serde_json::from_slice::<Value>(&body) {
        serde_json::to_vec_pretty(&json)?
    } else {
        body
    };

    io::copy(&mut out.as_slice(), &mut io::stdout())?;

    Ok(())
}

impl<'a> Exec<'a> for Command {
    fn exec(&self, args: &ArgMatches<'a>) -> Result<CommandResult> {
        if let Command::Init = self {
            Config::init_from_args(args)?;
            Ok(CommandResult::Empty)
        } else {
            let (cmd, args) = args.subcommand();
            let args = args.expect("sub-command args");
            #[cfg_attr(rustfmt, rustfmt_skip)]
            match self {
                Command::Campaign => cmd.parse::<Campaign>()?.exec(args),
                Command::Device   => cmd.parse::<Device>()?.exec(args),
                Command::Group    => cmd.parse::<Group>()?.exec(args),
                Command::Package  => cmd.parse::<Package>()?.exec(args),
                Command::Update   => cmd.parse::<Update>()?.exec(args),
                Command::Init     => unreachable!()
            }
        }
    }
}

impl FromStr for Command {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        match s.to_lowercase().as_ref() {
            "init"     => Ok(Command::Init),
            "campaign" => Ok(Command::Campaign),
            "device"   => Ok(Command::Device),
            "group"    => Ok(Command::Group),
            "package"  => Ok(Command::Package),
            "update"   => Ok(Command::Update),
            _ => Err(Error::Command(format!("unknown command: {}", s))),
        }
    }
}

/// Available campaign sub-commands.
#[derive(Serialize, Deserialize, PartialEq, Clone, Copy, Debug)]
pub enum Campaign {
    List,
    Create,
    Launch,
    Cancel,
    ListUpdates,
    CreateUpdate,
}

impl<'a> Exec<'a> for Campaign {
    fn exec(&self, args: &ArgMatches<'a>) -> Result<CommandResult> {
        let mut config = Config::load_default()?;
        let campaign = || args.value_of("campaign").expect("--campaign").parse();
        let update = || args.value_of("update").expect("--update").parse();
        let name = || args.value_of("name").expect("--name");
        let description = || args.value_of("description").expect("--description");

        #[cfg_attr(rustfmt, rustfmt_skip)]
        match self {
            Campaign::List   => Campaigner::list_from_args(&mut config, args),
            Campaign::Create => Campaigner::create_from_args(&mut config, args),
            Campaign::Launch => Campaigner::launch_campaign(&mut config, campaign()?),
            Campaign::Cancel => Campaigner::cancel_campaign(&mut config, campaign()?),
            Campaign::ListUpdates  => Campaigner::list_updates(&mut config,),
            Campaign::CreateUpdate  => Campaigner::create_update(&mut config, update()?, name(), description())
        }
            .map(|r| r.into())
    }
}

impl FromStr for Campaign {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        match s.to_lowercase().as_ref() {
            "list"   => Ok(Campaign::List),
            "create" => Ok(Campaign::Create),
            "launch" => Ok(Campaign::Launch),
            "cancel" => Ok(Campaign::Cancel),
            "createupdate" => Ok(Campaign::CreateUpdate),
            "listupdates" => Ok(Campaign::ListUpdates),
            _ => Err(Error::Command(format!("unknown campaign subcommand: {}", s))),
        }
    }
}

/// Available device sub-commands.
#[derive(Serialize, Deserialize, PartialEq, Clone, Copy, Debug)]
pub enum Device {
    List,
    Create,
    Delete,
}

impl<'a> Exec<'a> for Device {
    fn exec(&self, args: &ArgMatches<'a>) -> Result<CommandResult> {
        let mut config = Config::load_default()?;
        let device = || args.value_of("device").expect("--device").parse();
        let name = || args.value_of("name").expect("--name");
        let id = || args.value_of("id").expect("--id");

        #[cfg_attr(rustfmt, rustfmt_skip)]
        match self {
            Device::List   => Registry::list_device_args(&mut config, args),
            Device::Create => Registry::create_device(&mut config, name(), id(), DeviceType::from_args(args)?),
            Device::Delete => Registry::delete_device(&mut config, device()?),
        }
            .map(|r| r.into())
    }
}

impl FromStr for Device {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        match s.to_lowercase().as_ref() {
            "list"   => Ok(Device::List),
            "create" => Ok(Device::Create),
            "delete" => Ok(Device::Delete),
            _ => Err(Error::Command(format!("unknown device subcommand: {}", s))),
        }
    }
}

/// Available group sub-commands.
#[derive(Serialize, Deserialize, PartialEq, Clone, Copy, Debug)]
pub enum Group {
    List,
    Create,
    Add,
    Rename,
    Remove,
}

impl<'a> Exec<'a> for Group {
    fn exec(&self, args: &ArgMatches<'a>) -> Result<CommandResult> {
        let mut config = Config::load_default()?;
        let group = || args.value_of("group").expect("--group").parse();
        let device = || args.value_of("device").expect("--device").parse();
        let name = || args.value_of("name").expect("--name");

        #[cfg_attr(rustfmt, rustfmt_skip)]
        match self {
            Group::List   => Registry::list_group_args(&mut config, args),
            Group::Create => Registry::create_group(&mut config, name(), GroupType::Static),
            Group::Add    => Registry::add_to_group(&mut config, group()?, device()?),
            Group::Remove => Registry::remove_from_group(&mut config, group()?, device()?),
            Group::Rename => Registry::rename_group(&mut config, group()?, name()),
        }
            .map(|r| r.into())
    }
}

impl FromStr for Group {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        match s.to_lowercase().as_ref() {
            "list"   => Ok(Group::List),
            "create" => Ok(Group::Create),
            "add"    => Ok(Group::Add),
            "rename" => Ok(Group::Rename),
            "remove" => Ok(Group::Remove),
            _ => Err(Error::Command(format!("unknown group subcommand: {}", s))),
        }
    }
}

/// Available package sub-commands.
#[derive(Serialize, Deserialize, PartialEq, Clone, Copy, Debug)]
pub enum Package {
    List,
    Add,
    Fetch,
    Upload,
}

impl<'a> Exec<'a> for Package {
    fn exec(&self, args: &ArgMatches<'a>) -> Result<CommandResult> {
        let mut config = Config::load_default()?;
        let name = || args.value_of("name").expect("--name");
        let version = || args.value_of("version").expect("--version");
        let packages = || args.value_of("packages").expect("--packages");

        #[cfg_attr(rustfmt, rustfmt_skip)]
        match self {
            Package::List   => Reposerver::list_packages(&mut config),
            Package::Add    => Reposerver::add_package(&mut config, TufPackage::from_args(args)?),
            Package::Fetch  => Reposerver::get_package(&mut config, name(), version()),
            Package::Upload => Reposerver::add_packages(&mut config, TufPackages::from(TargetPackages::from_file(packages())?)?),
        }
            .map(|r| r.into())
    }
}

impl FromStr for Package {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        match s.to_lowercase().as_ref() {
            "list"   => Ok(Package::List),
            "add"    => Ok(Package::Add),
            "fetch"  => Ok(Package::Fetch),
            "upload" => Ok(Package::Upload),
            _ => Err(Error::Command(format!("unknown package subcommand: {}", s))),
        }
    }
}

/// Available update sub-commands.
#[derive(Serialize, Deserialize, PartialEq, Clone, Copy, Debug)]
pub enum Update {
    Create,
    Launch,
}

impl<'a> Exec<'a> for Update {
    fn exec(&self, args: &ArgMatches<'a>) -> Result<CommandResult> {
        let mut config = Config::load_default()?;
        let update = || args.value_of("update").expect("--update").parse();
        let device = || args.value_of("device").expect("--device").parse();
        let targets = || args.value_of("targets").expect("--targets");

        match self {
            Update::Create => Director::create_mtu(&mut config, &TufUpdates::from(TargetRequests::from_file(targets())?)?),
            Update::Launch => Director::launch_mtu(&mut config, update()?, device()?),
        }
        .map(|r| r.into())
    }
}

impl FromStr for Update {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        match s.to_lowercase().as_ref() {
            "create" => Ok(Update::Create),
            "launch" => Ok(Update::Launch),
            _ => Err(Error::Command(format!("unknown update subcommand: {}", s))),
        }
    }
}
