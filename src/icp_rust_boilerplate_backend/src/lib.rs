#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Product {
    id: u64,
    name: String,
    origin: String,
    current_location: String,
    status: String,  // e.g., "Manufactured", "In Transit", "Delivered"
    certification: Option<String>,
    timestamp: u64,
    last_update: Option<u64>,
    iot_data: Option<String>,  // Data from IoT sensors
}

// Implementing Storable for Product
impl Storable for Product {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

// Implementing BoundedStorable for Product
impl BoundedStorable for Product {
    const MAX_SIZE: u32 = 2048;  // Increased size for potential IoT data
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create an ID counter")
    );

    static PRODUCT_STORAGE: RefCell<StableBTreeMap<u64, Product, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct ProductPayload {
    name: String,
    origin: String,
    current_location: String,
    status: String,
    certification: Option<String>,
    iot_data: Option<String>,  // IoT data can be supplied here
}

// Query to retrieve a product by ID
#[ic_cdk::query]
fn get_product(id: u64) -> Result<Product, Error> {
    match _get_product(&id) {
        Some(product) => Ok(product),
        None => Err(Error::NotFound {
            msg: format!("Product with id={} not found", id),
        }),
    }
}

// Add a new product entry
#[ic_cdk::update]
fn add_product(product: ProductPayload) -> Option<Product> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment ID counter");
        
    let product = Product {
        id,
        name: product.name,
        origin: product.origin,
        current_location: product.current_location,
        status: product.status,
        certification: product.certification,
        timestamp: time(),
        last_update: None,
        iot_data: product.iot_data,
    };
    
    do_insert(&product);
    Some(product)
}

// Update product details (e.g., location, status, certification, IoT data)
#[ic_cdk::update]
fn update_product(id: u64, payload: ProductPayload) -> Result<Product, Error> {
    match PRODUCT_STORAGE.with(|storage| storage.borrow().get(&id)) {
        Some(mut product) => {
            product.current_location = payload.current_location;
            product.status = payload.status;
            product.certification = payload.certification;
            product.iot_data = payload.iot_data;
            product.last_update = Some(time());
            do_insert(&product);
            Ok(product)
        }
        None => Err(Error::NotFound {
            msg: format!("Cannot update product with id={}. Product not found", id),
        }),
    }
}

// Delete a product entry by ID
#[ic_cdk::update]
fn delete_product(id: u64) -> Result<Product, Error> {
    match PRODUCT_STORAGE.with(|storage| storage.borrow_mut().remove(&id)) {
        Some(product) => Ok(product),
        None => Err(Error::NotFound {
            msg: format!("Cannot delete product with id={}. Product not found.", id),
        }),
    }
}

// Helper method for inserting a product into storage
fn do_insert(product: &Product) {
    PRODUCT_STORAGE.with(|storage| storage.borrow_mut().insert(product.id, product.clone()));
}

// Helper method to retrieve a product by ID
fn _get_product(id: &u64) -> Option<Product> {
    PRODUCT_STORAGE.with(|storage| storage.borrow().get(id))
}

// Custom error handling
#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

// Candid export for interface generation
ic_cdk::export_candid!();
