use std::io::{BufRead, Write};

use niffler;
use quick_xml;
use quick_xml::{Reader, Writer};
use thiserror::Error;

use crate::metadata::{Repository, EVR};

pub struct RepomdXml;
pub struct PrimaryXml;
pub struct FilelistsXml;
pub struct OtherXml;
pub struct UpdateinfoXml;

pub const METADATA_PRIMARY: &str = "primary";
pub const METADATA_FILELISTS: &str = "filelists";
pub const METADATA_OTHER: &str = "other";
pub const METADATA_PRIMARY_DB: &str = "primary_db";
pub const METADATA_FILELISTS_DB: &str = "filelists_db";
pub const METADATA_OTHER_DB: &str = "other_db";
pub const METADATA_PRIMARY_ZCK: &str = "primary_zck";
pub const METADATA_FILELISTS_ZCK: &str = "filelists_zck";
pub const METADATA_OTHER_ZCK: &str = "other_zck";

#[derive(Error, Debug)]
pub enum MetadataError {
    #[error(transparent)]
    MetadataParseError(#[from] quick_xml::Error),
    #[error(transparent)]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    IntFieldParseError(#[from] std::num::ParseIntError),
    #[error(transparent)]
    UnsupportedCompressionTypeError(#[from] niffler::Error),
    #[error("Checksum type {0} is not supported")]
    UnsupportedChecksumTypeError(String),
    #[error("Missing metadata fields: {0}")]
    MissingFieldError(&'static str), // TODO: support multiple missing fields?
    #[error("Missing metadata attributes: {0}")]
    MissingAttributeError(&'static str), // TODO: support multiple missing attributes?
    #[error("Missing metadata header")]
    MissingHeaderError,
}

/// Default namespace for primary.xml
pub const XML_NS_COMMON: &str = "http://linux.duke.edu/metadata/common";
/// Default namespace for filelists.xml
pub const XML_NS_FILELISTS: &str = "http://linux.duke.edu/metadata/filelists";
/// Default namespace for other.xml
pub const XML_NS_OTHER: &str = "http://linux.duke.edu/metadata/other";
/// Default namespace for repomd.xml
pub const XML_NS_REPO: &str = "http://linux.duke.edu/metadata/repo";
/// Namespace for rpm (used in primary.xml and repomd.xml)
pub const XML_NS_RPM: &str = "http://linux.duke.edu/metadata/rpm";

pub trait RpmMetadata {
    fn filename() -> &'static str;

    fn load_metadata<R: BufRead>(
        repository: &mut Repository,
        buffer: &mut Reader<R>,
    ) -> Result<(), MetadataError>;

    fn write_metadata<W: Write>(
        repository: &Repository,
        buffer: &mut Writer<W>,
    ) -> Result<(), MetadataError>;
}

// TODO: Trait impl tests https://github.com/rust-lang/rfcs/issues/616

#[derive(Debug, Clone, Copy)]
pub enum CompressionType {
    None,
    Gzip,
    Xz,
    Bz2,
}

impl TryInto<CompressionType> for &str {
    type Error = MetadataError;

    fn try_into(self) -> Result<CompressionType, Self::Error> {
        match self {
            "gzip" => Ok(CompressionType::Gzip),
            "bz2" => Ok(CompressionType::Bz2),
            "xz" => Ok(CompressionType::Xz),
            "none" => Ok(CompressionType::None),
            _ => Err(MetadataError::UnsupportedChecksumTypeError(self.to_owned())),
        }
    }
}

// impl<T: Ord> Ord for Package {
//     #[inline]
//     fn cmp(&self, other: &Package) -> Ordering {
//         other.0.cmp(&self.0)
//     }
// }

#[derive(Debug, PartialEq, Default)]
pub struct Package {
    pub name: String,
    pub arch: String,
    pub evr: EVR,
    pub checksum: Checksum,
    pub location_href: String,
    pub summary: String,
    pub description: String,
    pub packager: String,
    pub url: String,
    pub time: Time,
    pub size: Size,

    pub rpm_license: String,           // rpm:license
    pub rpm_vendor: String,            // rpm:vendor
    pub rpm_group: String,             // rpm:group
    pub rpm_buildhost: String,         // rpm:buildhost
    pub rpm_sourcerpm: String,         // rpm:sourcerpm
    pub rpm_header_range: HeaderRange, // rpm:header-range

    pub rpm_requires: Vec<Requirement>,    // rpm:provides
    pub rpm_provides: Vec<Requirement>,    // rpm:requires
    pub rpm_conflicts: Vec<Requirement>,   // rpm:conflicts
    pub rpm_obsoletes: Vec<Requirement>,   // rpm:obsoletes
    pub rpm_suggests: Vec<Requirement>,    // rpm:suggests
    pub rpm_enhances: Vec<Requirement>,    // rpm:enhances
    pub rpm_recommends: Vec<Requirement>,  // rpm:recommends
    pub rpm_supplements: Vec<Requirement>, // rpm:supplements

    pub rpm_changelogs: Vec<Changelog>,
    pub rpm_files: Vec<PackageFile>,
}

impl Package {
    pub fn new(
        name: &str,
        version: &EVR,
        arch: &str,
        checksum: &Checksum,
        location_href: &str,
    ) -> Package {
        Package {
            name: name.to_owned(),
            arch: arch.to_owned(),
            evr: version.clone(), // TODO
            checksum: checksum.clone(),
            location_href: location_href.to_owned(),
            ..Package::default()
        }
    }

    pub fn nevra<'a>(&'a self) -> Nevra<'a> {
        self.into()
    }
}

pub struct Nevra<'a> {
    pub name: &'a str,
    pub arch: &'a str,
    pub evr: &'a EVR,
}

impl<'a> From<&'a Package> for Nevra<'a> {
    fn from(pkg: &'a Package) -> Self {
        Self {
            name: &pkg.name,
            evr: &pkg.evr,
            arch: &pkg.arch,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ChecksumType {
    Sha1,
    Sha256,
    Sha384,
    Sha512,
    Unknown,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Checksum {
    Sha1(String),
    Sha256(String),
    Sha384(String),
    Sha512(String),
    Unknown,
}

impl Default for Checksum {
    fn default() -> Self {
        Checksum::Unknown
    }
}

impl Checksum {
    pub fn try_create<N: AsRef<[u8]> + Sized>(
        checksum_type: N,
        checksum: N,
    ) -> Result<Self, MetadataError> {
        let bytes_to_str = |value| std::str::from_utf8(value).unwrap().to_owned();

        let checksum = match checksum_type.as_ref() {
            b"sha" => Checksum::Sha1(bytes_to_str(checksum.as_ref())),
            b"sha1" => Checksum::Sha1(bytes_to_str(checksum.as_ref())),
            b"sha256" => Checksum::Sha256(bytes_to_str(checksum.as_ref())),
            b"sha384" => Checksum::Sha384(bytes_to_str(checksum.as_ref())),
            b"sha512" => Checksum::Sha512(bytes_to_str(checksum.as_ref())),
            _ => {
                return Err(MetadataError::UnsupportedChecksumTypeError(bytes_to_str(
                    checksum_type.as_ref(),
                )))
            }
        };
        Ok(checksum)
    }

    pub fn to_values<'a>(&'a self) -> Result<(&str, &'a str), MetadataError> {
        let values = match self {
            Checksum::Sha1(c) => ("sha1", c.as_str()),
            Checksum::Sha256(c) => ("sha256", c.as_str()),
            Checksum::Sha384(c) => ("sha384", c.as_str()),
            Checksum::Sha512(c) => ("sha512", c.as_str()),
            Checksum::Unknown => panic!("Cannot take value of a checksum of unknown type"),
        };
        Ok(values)
    }
}

#[derive(Debug, PartialEq, Default)]
pub struct Changelog {
    pub author: String,
    pub date: u64,
    pub description: String,
}

#[derive(Debug, PartialEq, Default)]
pub struct Time {
    pub file: u64,
    pub build: u64,
}

#[derive(Debug, PartialEq, Default)]
pub struct Size {
    pub package: u64,
    pub installed: u64,
    pub archive: u64,
}

#[derive(Debug, PartialEq, Default)]
pub struct HeaderRange {
    pub start: u64,
    pub end: u64,
}

// Requirement (Provides, Conflicts, Obsoletes, Requires).
#[derive(Debug, PartialEq, Default)]
pub struct Requirement {
    pub name: String,
    pub flags: Option<String>,
    pub epoch: Option<String>,
    pub version: Option<String>,
    pub release: Option<String>,
    pub preinstall: Option<bool>,
}

pub enum RequirementType {
    LT,
    GT,
    EQ,
    LE,
    GE,
}

impl From<RequirementType> for &str {
    fn from(rtype: RequirementType) -> &'static str {
        match rtype {
            RequirementType::LT => "LT",
            RequirementType::GT => "GT",
            RequirementType::EQ => "EQ",
            RequirementType::LE => "LE",
            RequirementType::GE => "GE",
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum FileType {
    File,
    Dir,
    Ghost,
}

// TODO: this is unnecessary / not the best way
impl FileType {
    pub fn try_create<N: AsRef<[u8]> + Sized>(val: N) -> Result<Self, MetadataError> {
        let ftype = match val.as_ref() {
            b"dir" => FileType::Dir,
            b"ghost" => FileType::Ghost,
            b"file" => FileType::File,
            _ => panic!(),
        };
        Ok(ftype)
    }

    pub fn to_values(&self) -> &[u8] {
        match self {
            FileType::File => b"file",
            FileType::Dir => b"dir",
            FileType::Ghost => b"ghost",
        }
    }
}

impl Default for FileType {
    fn default() -> Self {
        FileType::File
    }
}

#[derive(Debug, PartialEq, Default)]
pub struct PackageFile {
    pub filetype: FileType,
    pub path: String,
}

#[derive(Debug, PartialEq)]
pub enum MetadataType {
    Primary,
    Filelists,
    Other,

    PrimaryZck,
    FilelistsZck,
    OtherZck,

    PrimaryDb,
    FilelistsDb,
    OtherDb,

    Unknown,
}

impl From<&str> for MetadataType {
    fn from(name: &str) -> Self {
        match name {
            METADATA_PRIMARY => MetadataType::Primary,
            METADATA_FILELISTS => MetadataType::Filelists,
            METADATA_OTHER => MetadataType::Other,

            METADATA_PRIMARY_DB => MetadataType::PrimaryDb,
            METADATA_FILELISTS_DB => MetadataType::FilelistsDb,
            METADATA_OTHER_DB => MetadataType::OtherDb,

            METADATA_PRIMARY_ZCK => MetadataType::PrimaryZck,
            METADATA_FILELISTS_ZCK => MetadataType::FilelistsZck,
            METADATA_OTHER_ZCK => MetadataType::OtherZck,

            _ => MetadataType::Unknown,
        }
    }
}

#[derive(Debug, PartialEq, Default)]
pub struct DistroTag {
    pub cpeid: Option<String>,
    pub name: String,
}

impl DistroTag {
    pub fn new(name: String, cpeid: Option<String>) -> Self {
        DistroTag { name, cpeid }
    }
}

#[derive(Debug, PartialEq, Default)]
pub struct RepoMdRecord {
    // TODO: location real? location base?  https://github.com/rpm-software-management/createrepo_c/commit/7e4ba3de1e9792f9d65f68c0d1cb18ed14ce1b68#diff-26e7fd2fdd746961fa628b1e9e42175640ec8d269c17e1608628d3377e0c07d4R371
    /// Record type
    pub mdtype: String,
    /// Relative location of the file in a repository
    pub location_href: String,
    /// Mtime of the file
    pub timestamp: u64,
    /// Size of the file
    pub size: Option<u64>,
    /// Checksum of the file
    pub checksum: Checksum,

    /// Size of the archive content
    pub open_size: Option<u64>,
    /// Checksum of the archive content
    pub open_checksum: Option<Checksum>,

    /// Size of the Zchunk header
    pub header_size: Option<u64>,
    /// Checksum of the Zchunk header
    pub header_checksum: Option<Checksum>,

    /// Database version (used only for sqlite databases like primary.sqlite etc.)
    pub database_version: Option<u32>,
}

#[derive(Debug, PartialEq, Default)]
pub struct UpdateRecord {
    pub from: String,
    pub update_type: String,
    pub status: String,
    pub version: String,
    pub id: String,
    pub title: String,
    pub issued_date: Option<String>,
    pub updated_date: Option<String>,
    pub rights: String,
    pub release: String,
    pub pushcount: Option<String>, // deprecated?
    pub severity: String,
    pub summary: String,
    pub description: String,
    pub solution: String,
    pub reboot_suggested: bool,
    pub references: Vec<UpdateReference>,
    pub pkglist: Vec<UpdateCollection>,
}

#[derive(Debug, PartialEq, Default)]
pub struct UpdateCollection {
    pub name: String,
    pub shortname: String,
    pub packages: Vec<UpdateCollectionPackage>,
}

#[derive(Debug, PartialEq, Default)]
pub struct UpdateReference {
    pub href: String,
    pub id: String,
    pub title: String,
    pub reftype: String,
}

#[derive(Debug, PartialEq, Default)]
pub struct UpdateCollectionPackage {
    pub epoch: u32,
    pub filename: String,
    pub name: String,
    pub reboot_suggested: bool,
    pub restart_suggested: bool,
    pub relogin_suggested: bool,
    pub release: String,
    pub src: String,
    pub checksum: Checksum,
    pub version: String,
}

#[derive(Debug, PartialEq, Default)]
pub struct UpdateCollectionModule {
    pub name: String,
    pub stream: String,
    pub version: u64,
    pub context: String,
    pub arch: String,
}

use rpm;
use std::convert::TryInto;

impl TryInto<Package> for rpm::RPMPackage {
    type Error = rpm::RPMError;

    fn try_into(self) -> Result<Package, Self::Error> {
        let pkg = Package {
            name: self.metadata.header.get_name()?.to_owned(),
            arch: self.metadata.header.get_arch()?.to_owned(),
            evr: {
                let epoch = self.metadata.header.get_epoch()?.to_string(); // TODO evaluate epoch type
                let version = self.metadata.header.get_version()?;
                let release = self.metadata.header.get_release()?;
                EVR::new(epoch.as_str(), version, release)
            },
            checksum: todo!(),
            location_href: todo!(),
            summary: todo!(),
            description: todo!(),
            packager: todo!(),
            url: todo!(),
            time: todo!(),
            size: todo!(),

            rpm_license: todo!(),
            rpm_vendor: todo!(),
            rpm_group: todo!(),
            rpm_buildhost: todo!(),
            rpm_sourcerpm: todo!(),
            rpm_header_range: todo!(),

            rpm_requires: todo!(),
            rpm_provides: todo!(),
            rpm_conflicts: todo!(),
            rpm_obsoletes: todo!(),
            rpm_suggests: todo!(),
            rpm_enhances: todo!(),
            rpm_recommends: todo!(),
            rpm_supplements: todo!(),

            rpm_changelogs: todo!(),
            rpm_files: todo!(),
        };

        Ok(pkg)
    }
}
