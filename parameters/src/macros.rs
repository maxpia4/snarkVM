// Copyright (C) 2019-2021 Aleo Systems Inc.
// This file is part of the snarkVM library.

// The snarkVM library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The snarkVM library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the snarkVM library. If not, see <https://www.gnu.org/licenses/>.

#[macro_export]
macro_rules! checksum {
    ($bytes: expr) => {
        hex::encode(snarkvm_algorithms::crh::sha256::sha256($bytes))
    };
}

#[macro_export]
macro_rules! checksum_error {
    ($expected: expr, $candidate: expr) => {
        Err(crate::errors::ParameterError::ChecksumMismatch($expected, $candidate))
    };
}

#[macro_export]
macro_rules! impl_local {
    ($name: ident, $local_dir: expr, $fname: tt, $ftype: tt) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name;

        impl $name {
            pub fn load_bytes() -> Result<Vec<u8>, crate::errors::ParameterError> {
                const METADATA: &'static str = include_str!(concat!($local_dir, $fname, ".metadata"));

                let metadata: serde_json::Value =
                    serde_json::from_str(METADATA).expect("Metadata was not well-formatted");
                let expected_checksum: String = metadata[concat!($ftype, "_checksum")]
                    .as_str()
                    .expect("Failed to parse checksum")
                    .to_string();
                let expected_size: usize = metadata[concat!($ftype, "_size")]
                    .to_string()
                    .parse()
                    .expect("Failed to retrieve the file size");

                let buffer = include_bytes!(concat!($local_dir, $fname, ".", $ftype));

                // Ensure the size matches.
                if expected_size != buffer.len() {
                    return Err(crate::errors::ParameterError::SizeMismatch(
                        expected_size,
                        buffer.len(),
                    ));
                }

                // Ensure the checksum matches.
                let candidate_checksum = checksum!(buffer);
                if expected_checksum != candidate_checksum {
                    return checksum_error!(expected_checksum, candidate_checksum);
                }

                return Ok(buffer.to_vec());
            }
        }

        paste::item! {
            #[cfg(test)]
            #[test]
            fn [< test_ $fname _ $ftype >]() {
                assert!($name::load_bytes().is_ok());
            }
        }
    };
}

#[macro_export]
macro_rules! impl_remote {
    ($name: ident, $remote_url: tt, $local_dir: expr, $fname: tt, $ftype: tt) => {
        pub struct $name;

        impl $name {
            pub fn load_bytes() -> Result<Vec<u8>, crate::errors::ParameterError> {
                const METADATA: &'static str = include_str!(concat!($local_dir, $fname, ".metadata"));

                let metadata: serde_json::Value =
                    serde_json::from_str(METADATA).expect("Metadata was not well-formatted");
                let expected_checksum: String = metadata[concat!($ftype, "_checksum")]
                    .as_str()
                    .expect("Failed to parse checksum")
                    .to_string();
                let expected_size: usize = metadata[concat!($ftype, "_size")]
                    .to_string()
                    .parse()
                    .expect("Failed to retrieve the file size");

                // Construct the versioned filename.
                let filename = match expected_checksum.get(0..7) {
                    Some(sum) => format!("{}.{}.{}", $fname, $ftype, sum),
                    _ => concat!($fname, $ftype).to_string(),
                };

                // Compose the correct file path for the parameter file.
                let mut file_path = std::path::PathBuf::from(file!());
                file_path.pop();
                file_path.push($local_dir);
                file_path.push(&filename);

                // Construct new parameters check.
                let parameters = crate::check_parameters::CheckParameters::new(
                    expected_checksum,
                    expected_size,
                    filename,
                    file_path,
                    String::from($remote_url),
                );

                // Compute parameters file bytes.
                let buffer = parameters.load_bytes()?;

                //
                // // Compute the relative path.
                // let relative_path = if file_path.strip_prefix("parameters").is_ok() {
                //     file_path.strip_prefix("parameters")?
                // } else {
                //     &file_path
                // };
                //
                // // Compute the absolute path.
                // let mut absolute_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                // absolute_path.push(&relative_path);
                //
                // // Compute the path to the aleo directory.
                // let mut aleo_path = aleo_std::aleo_dir();
                // aleo_path.push(&relative_path);
                //
                // let buffer = if aleo_path.exists() {
                //     // Attempts to load the parameter file locally with a path to the aleo directory.
                //     std::fs::read(aleo_path)?
                // } else if relative_path.exists() {
                //     // Attempts to load the parameter file locally with a relative path.
                //     std::fs::read(relative_path)?
                // } else if absolute_path.exists() {
                //     // Attempts to load the parameter file locally with an absolute path.
                //     std::fs::read(absolute_path)?
                // } else {
                //     // Downloads the missing parameters and stores it in the local directory for use.
                //     eprintln!(
                //         "\nWARNING - \"{}\" does not exist, downloading this file remotely and storing it locally. Please ensure \"{}\" is stored in {:?}.\n",
                //         filename, filename, file_path
                //     );
                //
                //     // Load remote file
                //     cfg_if::cfg_if! {
                //         if #[cfg(not(feature = "wasm"))] {
                //             #[cfg(not(feature = "no_std_out"))]
                //             println!("{} - Downloading parameters...", module_path!());
                //
                //
                //             let mut buffer = vec![];
                //             Self::remote_fetch(&mut buffer, &format!("{}/{}", $remote_url, filename))?;
                //
                //             #[cfg(not(feature = "no_std_out"))]
                //             println!("\n{} - Download complete", module_path!());
                //
                //             // Ensure the checksum matches.
                //             let candidate_checksum = checksum!(&buffer);
                //             if expected_checksum != candidate_checksum {
                //                 return checksum_error!(expected_checksum, candidate_checksum)
                //             }
                //
                //             match Self::store_bytes(&buffer, &aleo_path, &relative_path, &absolute_path, &file_path) {
                //                 Ok(()) => buffer,
                //                 Err(_) => {
                //                     eprintln!(
                //                         "\nWARNING - Failed to store \"{}\" locally. Please download this file manually and ensure it is stored in {:?}.\n",
                //                         filename, file_path
                //                     );
                //                     buffer
                //                 }
                //             }
                //         } else if #[cfg(feature = "wasm")] {
                //             let buffer = alloc::sync::Arc::new(parking_lot::RwLock::new(vec![]));
                //             let url = String::from($remote_url);
                //
                //             // NOTE(julesdesmit): I'm leaking memory here so that I can get a
                //             // static reference to the url, which is needed to pass it into
                //             // the local thread which downloads the file.
                //             let url = Box::leak(url.into_boxed_str());
                //
                //             let buffer_clone = alloc::sync::Arc::downgrade(&buffer);
                //             Self::remote_fetch(buffer_clone, url)?;
                //
                //             // Recover the bytes.
                //             let buffer = alloc::sync::Arc::try_unwrap(buffer).unwrap();
                //             let buffer = buffer.write().clone();
                //
                //             // Ensure the checksum matches.
                //             let candidate_checksum = checksum!(&buffer);
                //             if expected_checksum != candidate_checksum {
                //                 return checksum_error!(expected_checksum, candidate_checksum)
                //             }
                //
                //             buffer
                //         } else {
                //             return Err(crate::errors::ParameterError::RemoteFetchDisabled);
                //         }
                //     }
                // };
                //
                //  // Ensure the size matches.
                // if expected_size != buffer.len() {
                //     return Err(crate::errors::ParameterError::SizeMismatch(expected_size, buffer.len()));
                // }
                //
                // // Ensure the checksum matches.
                // let candidate_checksum = checksum!(buffer.as_slice());
                // if expected_checksum != candidate_checksum {
                //     return checksum_error!(expected_checksum, candidate_checksum)
                // }

                return Ok(buffer);
            }
            //
            // #[cfg(not(feature = "wasm"))]
            // fn store_bytes(
            //     buffer: &[u8],
            //     aleo_path: &std::path::Path,
            //     relative_path: &std::path::Path,
            //     absolute_path: &std::path::Path,
            //     file_path: &std::path::Path,
            // ) -> Result<(), crate::errors::ParameterError> {
            //     use snarkvm_utilities::Write;
            //
            //     // Hide compilation warning.
            //     let _ = file_path;
            //
            //     #[cfg(not(feature = "no_std_out"))]
            //     println!("{} - Storing parameters ({:?})", module_path!(), file_path);
            //
            //     // Attempt to write the parameter buffer to a file.
            //     if let Ok(mut file) = std::fs::File::create(aleo_path) {
            //         file.write_all(&buffer)?;
            //     } else if let Ok(mut file) = std::fs::File::create(relative_path) {
            //         file.write_all(&buffer)?;
            //     } else if let Ok(mut file) = std::fs::File::create(absolute_path) {
            //         file.write_all(&buffer)?;
            //     }
            //     Ok(())
            // }
            //
            // #[cfg(not(feature = "wasm"))]
            // fn remote_fetch(buffer: &mut Vec<u8>, url: &str) -> Result<(), crate::errors::ParameterError> {
            //     let mut easy = curl::easy::Easy::new();
            //     easy.url(url)?;
            //     #[cfg(not(feature = "no_std_out"))]
            //     {
            //         easy.progress(true)?;
            //         easy.progress_function(|total_download, current_download, _, _| {
            //             let percent = (current_download / total_download) * 100.0;
            //             let size_in_megabytes = total_download as u64 / 1_048_576;
            //             print!(
            //                 "\r{} - {:.2}% complete ({:#} MB total)",
            //                 module_path!(),
            //                 percent,
            //                 size_in_megabytes
            //             );
            //             true
            //         })?;
            //     }
            //
            //     let mut transfer = easy.transfer();
            //     transfer.write_function(|data| {
            //         buffer.extend_from_slice(data);
            //         Ok(data.len())
            //     })?;
            //     Ok(transfer.perform()?)
            // }
            //
            // #[cfg(feature = "wasm")]
            // fn remote_fetch(buffer: alloc::sync::Weak<parking_lot::RwLock<Vec<u8>>>, url: &'static str) -> Result<(), crate::errors::ParameterError> {
            //     // NOTE(julesdesmit): We spawn a local thread here in order to be
            //     // able to accommodate the async syntax from reqwest.
            //     wasm_bindgen_futures::spawn_local(async move {
            //         let content = reqwest::get(url)
            //             .await
            //             .unwrap()
            //             .text()
            //             .await
            //             .unwrap();
            //
            //         let buffer = buffer.upgrade().unwrap();
            //         buffer
            //             .write()
            //             .extend_from_slice(content.as_bytes());
            //         drop(buffer);
            //     });
            //     Ok(())
            // }
        }

        paste::item! {
            #[cfg(test)]
            #[test]
            fn [< test_ $fname _ $ftype >]() {
                assert!($name::load_bytes().is_ok());
            }
        }
    };
}

// // Copyright (C) 2019-2021 Aleo Systems Inc.
// // This file is part of the snarkVM library.
//
// // The snarkVM library is free software: you can redistribute it and/or modify
// // it under the terms of the GNU General Public License as published by
// // the Free Software Foundation, either version 3 of the License, or
// // (at your option) any later version.
//
// // The snarkVM library is distributed in the hope that it will be useful,
// // but WITHOUT ANY WARRANTY; without even the implied warranty of
// // MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// // GNU General Public License for more details.
//
// // You should have received a copy of the GNU General Public License
// // along with the snarkVM library. If not, see <https://www.gnu.org/licenses/>.
//
// #[macro_export]
// macro_rules! checksum {
//     ($bytes: expr) => {
//         hex::encode(snarkvm_algorithms::crh::sha256::sha256($bytes))
//     };
// }
//
// #[macro_export]
// macro_rules! checksum_error {
//     ($expected: expr, $candidate: expr) => {
//         Err(crate::errors::ParameterError::ChecksumMismatch($expected, $candidate))
//     };
// }
//
// #[macro_export]
// macro_rules! impl_local {
//     ($name: ident, $local_dir: expr, $fname: tt, $ftype: tt) => {
//         #[derive(Clone, Debug, PartialEq, Eq)]
//         pub struct $name;
//
//         impl $name {
//             pub fn load_bytes() -> Result<Vec<u8>, crate::errors::ParameterError> {
//                 const METADATA: &'static str = include_str!(concat!($local_dir, $fname, ".metadata"));
//
//                 let metadata: serde_json::Value =
//                     serde_json::from_str(METADATA).expect("Metadata was not well-formatted");
//                 let expected_checksum: String = metadata[concat!($ftype, "_checksum")]
//                     .as_str()
//                     .expect("Failed to parse checksum")
//                     .to_string();
//                 let expected_size: usize = metadata[concat!($ftype, "_size")]
//                     .to_string()
//                     .parse()
//                     .expect("Failed to retrieve the file size");
//
//                 let buffer = include_bytes!(concat!($local_dir, $fname, ".", $ftype));
//
//                 // Ensure the size matches.
//                 if expected_size != buffer.len() {
//                     return Err(crate::errors::ParameterError::SizeMismatch(
//                         expected_size,
//                         buffer.len(),
//                     ));
//                 }
//
//                 // Ensure the checksum matches.
//                 let candidate_checksum = checksum!(buffer);
//                 if expected_checksum != candidate_checksum {
//                     return checksum_error!(expected_checksum, candidate_checksum);
//                 }
//
//                 return Ok(buffer.to_vec());
//             }
//         }
//
//         paste::item! {
//             #[cfg(test)]
//             #[test]
//             fn [< test_ $fname _ $ftype >]() {
//                 assert!($name::load_bytes().is_ok());
//             }
//         }
//     };
// }
//
// #[macro_export]
// macro_rules! impl_remote {
//     ($name: ident, $remote_url: tt, $local_dir: expr, $filename: tt, $filetype: tt) => {
//         pub struct $name;
//
//         impl $name {
//             pub fn load_bytes() -> Result<Vec<u8>, crate::errors::ParameterError> {
//                 const METADATA: &'static str = include_str!(concat!($local_dir, $filename, ".metadata"));
//
//                 let metadata: serde_json::Value =
//                     serde_json::from_str(METADATA).expect("Metadata was not well-formatted");
//                 let expected_checksum: String = metadata[concat!($filetype, "_checksum")]
//                     .as_str()
//                     .expect("Failed to parse checksum")
//                     .to_string();
//                 let expected_size: usize = metadata[concat!($filetype, "_size")]
//                     .to_string()
//                     .parse()
//                     .expect("Failed to retrieve the file size");
//
//                 // Construct the versioned filename.
//                 let filename = match expected_checksum.get(0..7) {
//                     Some(sum) => format!("{}.{}.{}", $filename, $filetype, sum),
//                     _ => concat!($filename, $filetype).to_string(),
//                 };
//
//                 // Compose the correct file path for the parameter file.
//                 let mut filepath = std::path::PathBuf::from(file!());
//                 filepath.pop();
//                 filepath.push($local_dir);
//                 filepath.push(&filename);
//
//                 // Load parameters.
//                 let parameters =
//                     crate::check_parameters::CheckParameters::new($filename, $filetype, $local_dir, $remote_url, &expected_checksum);
//                 let buffer = parameters.load_bytes()?;
//
//                 // Ensure the size matches.
//                 if expected_size != buffer.len() {
//                     return Err(crate::errors::ParameterError::SizeMismatch(
//                         expected_size,
//                         buffer.len(),
//                     ));
//                 }
//
//                 // Ensure the checksum matches.
//                 let candidate_checksum = checksum!(buffer.as_slice());
//                 if expected_checksum != candidate_checksum {
//                     return checksum_error!(expected_checksum, candidate_checksum);
//                 }
//
//                 return Ok(buffer);
//
//                 //
//                 // // Compute the relative path.
//                 // let relative_path = if file_path.strip_prefix("parameters").is_ok() {
//                 //     file_path.strip_prefix("parameters")?
//                 // } else {
//                 //     &file_path
//                 // };
//                 //
//                 // // Compute the absolute path.
//                 // let mut absolute_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
//                 // absolute_path.push(&relative_path);
//                 //
//                 // let buffer = if relative_path.exists() {
//                 //     // Attempts to load the parameter file locally with a relative path.
//                 //     std::fs::read(relative_path)?
//                 // } else if absolute_path.exists() {
//                 //     // Attempts to load the parameter file locally with an absolute path.
//                 //     std::fs::read(absolute_path)?
//                 // } else {
//                 //     // Downloads the missing parameters and stores it in the local directory for use.
//                 //     eprintln!(
//                 //         "\nWARNING - \"{}\" does not exist, downloading this file remotely and storing it locally. Please ensure \"{}\" is stored in {:?}.\n",
//                 //         filename, filename, file_path
//                 //     );
//                 //
//                 //     // Load remote file
//                 //     cfg_if::cfg_if! {
//                 //         if #[cfg(not(feature = "wasm"))] {
//                 //             println!("{} - Downloading parameters...", module_path!());
//                 //             let mut buffer = vec![];
//                 //             Self::remote_fetch(&mut buffer, &format!("{}/{}", $remote_url, filename))?;
//                 //             println!("\n{} - Download complete", module_path!());
//                 //
//                 //             // Ensure the checksum matches.
//                 //             let candidate_checksum = checksum!(&buffer);
//                 //             if expected_checksum != candidate_checksum {
//                 //                 return checksum_error!(expected_checksum, candidate_checksum)
//                 //             }
//                 //
//                 //             match Self::store_bytes(&buffer, &relative_path, &absolute_path, &file_path) {
//                 //                 Ok(()) => buffer,
//                 //                 Err(_) => {
//                 //                     eprintln!(
//                 //                         "\nWARNING - Failed to store \"{}\" locally. Please download this file manually and ensure it is stored in {:?}.\n",
//                 //                         filename, file_path
//                 //                     );
//                 //                     buffer
//                 //                 }
//                 //             }
//                 //         } else if #[cfg(feature = "wasm")] {
//                 //             let buffer = alloc::sync::Arc::new(parking_lot::RwLock::new(vec![]));
//                 //             let url = String::from($remote_url);
//                 //
//                 //             // NOTE(julesdesmit): I'm leaking memory here so that I can get a
//                 //             // static reference to the url, which is needed to pass it into
//                 //             // the local thread which downloads the file.
//                 //             let url = Box::leak(url.into_boxed_str());
//                 //
//                 //             let buffer_clone = alloc::sync::Arc::downgrade(&buffer);
//                 //             Self::remote_fetch(buffer_clone, url)?;
//                 //
//                 //             // Recover the bytes.
//                 //             let buffer = alloc::sync::Arc::try_unwrap(buffer).unwrap();
//                 //             let buffer = buffer.write().clone();
//                 //
//                 //             // Ensure the checksum matches.
//                 //             let candidate_checksum = checksum!(&buffer);
//                 //             if expected_checksum != candidate_checksum {
//                 //                 return checksum_error!(expected_checksum, candidate_checksum)
//                 //             }
//                 //
//                 //             buffer
//                 //         } else {
//                 //             return Err(crate::errors::ParameterError::RemoteFetchDisabled);
//                 //         }
//                 //     }
//                 // };
//             }
//             //
//             // #[cfg(not(feature = "wasm"))]
//             // fn store_bytes(
//             //     buffer: &[u8],
//             //     relative_path: &std::path::Path,
//             //     absolute_path: &std::path::Path,
//             //     file_path: &std::path::Path,
//             // ) -> Result<(), crate::errors::ParameterError> {
//             //     use snarkvm_utilities::Write;
//             //
//             //     println!("{} - Storing parameters ({:?})", module_path!(), file_path);
//             //     // Attempt to write the parameter buffer to a file.
//             //     if let Ok(mut file) = std::fs::File::create(relative_path) {
//             //         file.write_all(&buffer)?;
//             //     } else if let Ok(mut file) = std::fs::File::create(absolute_path) {
//             //         file.write_all(&buffer)?;
//             //     }
//             //     Ok(())
//             // }
//             //
//             // #[cfg(not(feature = "wasm"))]
//             // fn remote_fetch(buffer: &mut Vec<u8>, url: &str) -> Result<(), crate::errors::ParameterError> {
//             //     let mut easy = curl::easy::Easy::new();
//             //     easy.url(url)?;
//             //     easy.progress(true)?;
//             //     easy.progress_function(|total_download, current_download, _, _| {
//             //         let percent = (current_download / total_download) * 100.0;
//             //         let size_in_megabytes = total_download as u64 / 1_048_576;
//             //         print!(
//             //             "\r{} - {:.2}% complete ({:#} MB total)",
//             //             module_path!(),
//             //             percent,
//             //             size_in_megabytes
//             //         );
//             //         true
//             //     })?;
//             //
//             //     let mut transfer = easy.transfer();
//             //     transfer.write_function(|data| {
//             //         buffer.extend_from_slice(data);
//             //         Ok(data.len())
//             //     })?;
//             //     Ok(transfer.perform()?)
//             // }
//             //
//             // #[cfg(feature = "wasm")]
//             // fn remote_fetch(buffer: alloc::sync::Weak<parking_lot::RwLock<Vec<u8>>>, url: &'static str) -> Result<(), crate::errors::ParameterError> {
//             //     // NOTE(julesdesmit): We spawn a local thread here in order to be
//             //     // able to accommodate the async syntax from reqwest.
//             //     wasm_bindgen_futures::spawn_local(async move {
//             //         let content = reqwest::get(url)
//             //             .await
//             //             .unwrap()
//             //             .text()
//             //             .await
//             //             .unwrap();
//             //
//             //         let buffer = buffer.upgrade().unwrap();
//             //         buffer
//             //             .write()
//             //             .extend_from_slice(content.as_bytes());
//             //         drop(buffer);
//             //     });
//             //     Ok(())
//             // }
//         }
//
//         paste::item! {
//             #[cfg(test)]
//             #[test]
//             fn [< test_ $filename _ $filetype >]() {
//                 assert!($name::load_bytes().is_ok());
//             }
//         }
//     };
// }
