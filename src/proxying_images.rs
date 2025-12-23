// pub async fn fetch_img_from_base_api(image_id: String) -> Vec<u8> {}
//
// pub async fn preload_image(image_id: String, content: Vec<u8>){
//
// }
//
//
// // TODO: а сейчас нужно садиться за какой-никакой design API
// //       - делаем processing_api, с pipeline форматом (builder)
// //       - добавляем storage_api для processing_api (по builder параметрам и image_id)
// //         - сейчас это будет caching storage, в последующем - что угодно
// //       - переделываем этот файл под fetcher trait, чтобы можно было менять форматы получения файлов
// //       - storage_api отвечает за хранение оригинальных изображений, нр должен быть и cache api - для хранения различных версий

