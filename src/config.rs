//! Configuration objects.

use std::{ffi::c_int, mem, ptr, sync::Arc, vec};

use crate::{
    check, check_log,
    display::DisplayOwner,
    raw::VAConfigID,
    surface::{RTFormat, SurfaceAttributes},
    Display, Entrypoint, Profile, Result, VAError, VAStatus,
};

ffi_enum! {
    pub enum ConfigAttribType: c_int {
        RTFormat          = 0,
        SpatialResidual   = 1,
        SpatialClipping   = 2,
        IntraResidual     = 3,
        Encryption        = 4,
        RateControl       = 5,
        DecSliceMode      = 6,
        DecJPEG           = 7,
        DecProcessing     = 8,
        EncPackedHeaders  = 10,
        EncInterlaced     = 11,
        EncMaxRefFrames   = 13,
        EncMaxSlices      = 14,
        EncSliceStructure = 15,
        EncMacroblockInfo = 16,
        MaxPictureWidth   = 18,
        MaxPictureHeight  = 19,
        EncJPEG             = 20,
        EncQualityRange     = 21,
        EncQuantization     = 22,
        EncIntraRefresh     = 23,
        EncSkipFrame        = 24,
        EncROI              = 25,
        EncRateControlExt   = 26,
        ProcessingRate      = 27,
        EncDirtyRect        = 28,
        EncParallelRateControl    = 29,
        EncDynamicScaling         = 30,
        FrameSizeToleranceSupport = 31,
        FEIFunctionType       = 32,
        FEIMVPredictors       = 33,
        Stats                 = 34,
        EncTileSupport        = 35,
        CustomRoundingControl = 36,
        QPBlockSize           = 37,
        MaxFrameSize          = 38,
        PredictionDirection   = 39,
        MultipleFrame         = 40,
        ContextPriority       = 41,
        DecAV1Features        = 42,
        TEEType               = 43,
        TEETypeClient         = 44,
        ProtectedContentCipherAlgorithm  = 45,
        ProtectedContentCipherBlockSize  = 46,
        ProtectedContentCipherMode       = 47,
        ProtectedContentCipherSampleType = 48,
        ProtectedContentUsage = 49,
        EncHEVCFeatures       = 50,
        EncHEVCBlockSizes     = 51,
        EncAV1                = 52,
        EncAV1Ext1            = 53,
        EncAV1Ext2            = 54,
        EncPerBlockControl    = 55,
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct ConfigAttrib {
    type_: ConfigAttribType,
    value: u32,
}

impl ConfigAttrib {
    fn zeroed() -> Self {
        unsafe { mem::zeroed() }
    }

    #[inline]
    pub fn attrib_type(&self) -> ConfigAttribType {
        self.type_
    }

    #[inline]
    pub fn raw_value(&self) -> u32 {
        self.value
    }
}

#[non_exhaustive]
pub enum ConfigAttribEnum {
    RTFormat(RTFormat),
}

impl From<ConfigAttribEnum> for ConfigAttrib {
    fn from(value: ConfigAttribEnum) -> Self {
        match value {
            ConfigAttribEnum::RTFormat(fmt) => ConfigAttrib {
                type_: ConfigAttribType::RTFormat,
                value: fmt.bits(),
            },
        }
    }
}

/// A codec configuration for a specific [`Entrypoint`] and [`Profile`].
pub struct Config {
    pub(crate) d: Arc<DisplayOwner>,
    pub(crate) id: VAConfigID,
}

impl Config {
    pub fn new(display: &Display, profile: Profile, entrypoint: Entrypoint) -> Result<Self> {
        Self::with_attribs(display, profile, entrypoint, &mut [])
    }

    pub fn with_attribs(
        display: &Display,
        profile: Profile,
        entrypoint: Entrypoint,
        attribs: &mut [ConfigAttrib],
    ) -> Result<Self> {
        unsafe {
            let mut config_id = 0;
            check(display.d.libva.vaCreateConfig(
                display.d.raw,
                profile,
                entrypoint,
                attribs.as_mut_ptr(),
                attribs.len().try_into().unwrap(),
                &mut config_id,
            ))?;
            Ok(Config {
                d: display.d.clone(),
                id: config_id,
            })
        }
    }

    pub fn query_surface_attributes(&self) -> Result<SurfaceAttributes> {
        unsafe {
            let mut num_attribs = 0;
            let status = self.d.libva.vaQuerySurfaceAttributes(
                self.d.raw,
                self.id,
                ptr::null_mut(),
                &mut num_attribs,
            );
            if status != VAStatus::SUCCESS && status != VAError::ERROR_MAX_NUM_EXCEEDED {
                return Err(check(status).unwrap_err());
            }

            let mut attribs = Vec::with_capacity(num_attribs as usize);
            check(self.d.libva.vaQuerySurfaceAttributes(
                self.d.raw,
                self.id,
                attribs.as_mut_ptr(),
                &mut num_attribs,
            ))?;
            attribs.set_len(num_attribs as usize);
            Ok(SurfaceAttributes { vec: attribs })
        }
    }

    pub fn query_config_attributes(&self) -> Result<ConfigAttributes> {
        let num_attribs = unsafe { self.d.libva.vaMaxNumConfigAttributes(self.d.raw) as usize };

        let mut profile = Profile(0);
        let mut entrypoint = Entrypoint(0);
        let mut attrib_list = vec![ConfigAttrib::zeroed(); num_attribs];
        let mut num_attribs = 0;
        unsafe {
            check(self.d.libva.vaQueryConfigAttributes(
                self.d.raw,
                self.id,
                &mut profile,
                &mut entrypoint,
                attrib_list.as_mut_ptr(),
                &mut num_attribs,
            ))?;
        }
        attrib_list.truncate(num_attribs as usize);
        attrib_list.shrink_to_fit();

        Ok(ConfigAttributes {
            profile,
            entrypoint,
            attribs: attrib_list,
        })
    }
}

impl Drop for Config {
    fn drop(&mut self) {
        unsafe {
            check_log(
                self.d.libva.vaDestroyConfig(self.d.raw, self.id),
                "vaDestroyConfig call in drop",
            );
        }
    }
}

pub struct ConfigAttributes {
    profile: Profile,
    entrypoint: Entrypoint,
    attribs: Vec<ConfigAttrib>,
}

impl ConfigAttributes {
    pub fn profile(&self) -> Profile {
        self.profile
    }

    pub fn entrypoint(&self) -> Entrypoint {
        self.entrypoint
    }

    pub fn len(&self) -> usize {
        self.attribs.len()
    }
}

impl IntoIterator for ConfigAttributes {
    type Item = ConfigAttrib;
    type IntoIter = vec::IntoIter<ConfigAttrib>;

    fn into_iter(self) -> Self::IntoIter {
        self.attribs.into_iter()
    }
}
