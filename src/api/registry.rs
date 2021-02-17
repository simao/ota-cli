use clap::ArgMatches;
use reqwest::blocking::{Client, Response};
use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};
use uuid::Uuid;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::http::{Http, HttpMethods};


/// Available Device Registry API methods.
pub trait RegistryApi {
    fn create_device(_: &mut Config, name: &str, id: &str, kind: DeviceType) -> Result<Response>;
    fn delete_device(_: &mut Config, device: Uuid) -> Result<Response>;
    fn list_device(_: &mut Config, device: Uuid) -> Result<Response>;
    fn list_all_devices(_: &mut Config) -> Result<Response>;

    fn create_group(_: &mut Config, name: &str, group_type: GroupType) -> Result<Response>;
    fn rename_group(_: &mut Config, group: Uuid, name: &str) -> Result<Response>;
    fn add_to_group(_: &mut Config, group: Uuid, device: Uuid) -> Result<Response>;
    fn remove_from_group(_: &mut Config, group: Uuid, device: Uuid) -> Result<Response>;

    fn list_groups(_: &mut Config, device: Uuid) -> Result<Response>;
    fn list_devices(_: &mut Config, group: Uuid) -> Result<Response>;
    fn list_all_groups(_: &mut Config) -> Result<Response>;
}


/// Make API calls to manage device groups.
pub struct Registry;

impl<'a> Registry {
    /// Parse args as device listing preferences.
    pub fn list_device_args(config: &mut Config, args: &ArgMatches<'a>) -> Result<Response> {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        match parse_list_args(args)? {
            (true, _, _)         => Self::list_all_devices(config),
            (_, Some(device), _) => Self::list_device(config, device),
            (_, _, Some(group))  => Self::list_devices(config, group),
            _ => Err(Error::Args("one of --all, --device, or --group required".into())),
        }
    }

    /// Parse args as group listing preferences.
    pub fn list_group_args(config: &mut Config, args: &ArgMatches<'a>) -> Result<Response> {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        match parse_list_args(args)? {
            (true, _, _)         => Self::list_all_groups(config),
            (_, Some(device), _) => Self::list_groups(config, device),
            (_, _, Some(group))  => Self::list_devices(config, group),
            _ => Err(Error::Args("one of --all, --device, or --group required".into())),
        }
    }
}

impl RegistryApi for Registry {
    fn create_device(config: &mut Config, name: &str, id: &str, kind: DeviceType) -> Result<Response> {
        debug!("creating device {} of type {} with id {}", name, kind, id);
        let req = Client::new().post(&format!("{}api/v1/devices", config.registry)).query(&[
            ("deviceName", name),
            ("deviceId", id),
            ("deviceType", &format!("{}", kind)),
        ]);
        Http::send(req, config.token()?)
    }

    fn delete_device(config: &mut Config, device: Uuid) -> Result<Response> {
        debug!("deleting device {}", device);
        Http::delete(&format!("{}api/v1/devices/{}", config.registry, device), config.token()?)
    }

    fn list_device(config: &mut Config, device: Uuid) -> Result<Response> {
        debug!("listing details for device {}", device);
        Http::get(&format!("{}api/v1/devices/{}", config.registry, device), config.token()?)
    }

    fn list_all_devices(config: &mut Config) -> Result<Response> {
        debug!("listing all devices");
        Http::get(&format!("{}api/v1/devices", config.registry), config.token()?)
    }

    fn create_group(config: &mut Config, name: &str, group_type: GroupType) -> Result<Response> {
        debug!("creating device group {}", name);
        let req = Client::new()
            .post(&format!("{}api/v1/device_groups", config.registry))
            .json(&json!({"name": name, "groupType": format!("{}", group_type)}));
        Http::send(req, config.token()?)
    }

    fn rename_group(config: &mut Config, group: Uuid, name: &str) -> Result<Response> {
        debug!("renaming group {} to {}", group, name);
        let req = Client::new()
            .put(&format!("{}api/v1/device_groups/{}/rename", config.registry, group))
            .query(&[("groupId", &format!("{}", group), ("groupName", name))]);
        Http::send(req, config.token()?)
    }

    fn add_to_group(config: &mut Config, group: Uuid, device: Uuid) -> Result<Response> {
        debug!("adding device {} to group {}", device, group);
        let req = Client::new()
            .post(&format!("{}api/v1/device_groups/{}/devices/{}", config.registry, group, device))
            .query(&[("deviceId", device), ("groupId", group)]);
        Http::send(req, config.token()?)
    }

    fn remove_from_group(config: &mut Config, group: Uuid, device: Uuid) -> Result<Response> {
        debug!("removing device {} from group {}", device, group);
        let req = Client::new()
            .delete(&format!("{}api/v1/device_groups/{}/devices/{}", config.registry, group, device))
            .query(&[("deviceId", format!("{}", device)), ("groupId", format!("{}", group))]);
        Http::send(req, config.token()?)
    }

    fn list_devices(config: &mut Config, group: Uuid) -> Result<Response> {
        debug!("listing devices in group {}", group);
        Http::get(
            &format!("{}api/v1/device_groups/{}/devices", config.registry, group),
            config.token()?,
        )
    }

    fn list_groups(config: &mut Config, device: Uuid) -> Result<Response> {
        debug!("listing groups for device {}", device);
        Http::get(&format!("{}api/v1/devices/{}/groups", config.registry, device), config.token()?)
    }

    fn list_all_groups(config: &mut Config) -> Result<Response> {
        debug!("listing all groups");
        Http::get(&format!("{}api/v1/device_groups", config.registry), config.token()?)
    }
}


/// Available device types.
#[derive(Clone, Copy, Debug)]
pub enum DeviceType {
    Vehicle,
    Other,
}

impl<'a> DeviceType {
    /// Parse CLI arguments into a `DeviceType`.
    pub fn from_args(args: &ArgMatches<'a>) -> Result<Self> {
        if args.is_present("vehicle") {
            Ok(DeviceType::Vehicle)
        } else if args.is_present("other") {
            Ok(DeviceType::Other)
        } else {
            Err(Error::Args("Either --vehicle or --other flag is required".into()))
        }
    }
}

impl FromStr for DeviceType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        match s.to_lowercase().as_ref() {
            "vehicle" => Ok(DeviceType::Vehicle),
            "other"   => Ok(DeviceType::Other),
            _ => Err(Error::Parse(format!("unknown `DeviceType`: {}", s))),
        }
    }
}

impl Display for DeviceType {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        let text = match self {
            DeviceType::Vehicle => "Vehicle",
            DeviceType::Other   => "Other",
        };
        write!(f, "{}", text)
    }
}


/// Available group types.
#[derive(Clone, Copy, Debug)]
pub enum GroupType {
    Static,
    Dynamic,
}

impl FromStr for GroupType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        match s.to_lowercase().as_ref() {
            "static"  => Ok(GroupType::Static),
            "dynamic" => Ok(GroupType::Dynamic),
            _ => Err(Error::Parse(format!("unknown `GroupType`: {}", s))),
        }
    }
}

impl Display for GroupType {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        let text = match self {
            GroupType::Static  => "static",
            GroupType::Dynamic => "dynamic",
        };
        write!(f, "{}", text)
    }
}


/// Parse into a tuple of --all, --device, and --group arg values.
fn parse_list_args<'a>(args: &ArgMatches<'a>) -> Result<(bool, Option<Uuid>, Option<Uuid>)> {
    let all = args.is_present("all");
    let device = if let Some(val) = args.value_of("device") {
        Some(val.parse()?)
    } else {
        None
    };
    let group = if let Some(val) = args.value_of("group") {
        Some(val.parse()?)
    } else {
        None
    };
    Ok((all, device, group))
}
