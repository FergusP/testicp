#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

// Define types for memory and ID cell
type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

// Define the Product structure
#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Product {
    id: u64,  // Unique identifier for the product
    name: String,  // Name of the product
    origin: String,  // Origin of the product
    current_location: String,  // Current location of the product
    status: String,  // Current status of the product (e.g., "Manufactured", "In Transit", "Delivered")
    certification: Option<String>,  // Optional certification information
    timestamp: u64,  // Timestamp of product creation
    last_update: Option<u64>,  // Optional last update timestamp
    iot_data: Option<String>,  // Optional data from IoT sensors
}

// Implementing Storable for Product to convert to/from bytes
impl Storable for Product {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())  // Convert Product to bytes using candid encoding
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()  // Decode bytes back to Product
    }
}

// Implementing BoundedStorable for Product with size limits
impl BoundedStorable for Product {
    const MAX_SIZE: u32 = 2048;  // Maximum size for Product data
    const IS_FIXED_SIZE: bool = false;  // Not a fixed size
}

// Thread-local storage
thread_local! {
    // Memory manager for stable memory
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    // ID counter for generating unique product IDs
    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create an ID counter")  // Panic if ID counter cannot be created
    );

    // Storage for products using a StableBTreeMap
    static PRODUCT_STORAGE: RefCell<StableBTreeMap<u64, Product, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

// Define the structure for payload when adding or updating a product
#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct ProductPayload {
    name: String,  // Name of the product
    origin: String,  // Origin of the product
    current_location: String,  // Current location of the product
    status: String,  // Current status of the product
    certification: Option<String>,  // Optional certification information
    iot_data: Option<String>,  // Optional IoT data
}

// Payload validation function for ProductPayload
fn validate_product_payload(payload: &ProductPayload) -> Result<(), Error> {
    // Validate that the name is not empty
    if payload.name.trim().is_empty() {
        return Err(Error::InvalidInput { msg: "Product name cannot be empty".to_string() });
    }
    // Validate that the origin is not empty
    if payload.origin.trim().is_empty() {
        return Err(Error::InvalidInput { msg: "Product origin cannot be empty".to_string() });
    }
    // Validate that the current location is not empty
    if payload.current_location.trim().is_empty() {
        return Err(Error::InvalidInput { msg: "Current location cannot be empty".to_string() });
    }
    // Validate that the status is not empty
    if payload.status.trim().is_empty() {
        return Err(Error::InvalidInput { msg: "Status cannot be empty".to_string() });
    }
    Ok(())
}

// Query to retrieve a product by ID
#[ic_cdk::query]
fn get_product(id: u64) -> Result<Product, Error> {
    match _get_product(&id) {
        Some(product) => Ok(product),  // Product found, return it
        None => Err(Error::NotFound {
            msg: format!("Product with id={} not found", id),  // Return not found error if product doesn't exist
        }),
    }
}

// Add a new product entry
#[ic_cdk::update]
fn add_product(product: ProductPayload) -> Result<Product, Error> {
    // Validate the product payload
    validate_product_payload(&product)?;

    // Generate a new unique ID for the product
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)  // Increment ID counter
        })
        .expect("Cannot increment ID counter");

    // Create a new product instance
    let product = Product {
        id,
        name: product.name,
        origin: product.origin,
        current_location: product.current_location,
        status: product.status,
        certification: product.certification,
        timestamp: time(),  // Capture the current timestamp
        last_update: None,  // No last update initially
        iot_data: product.iot_data,
    };

    // Insert the product into storage
    do_insert(&product);
    Ok(product)  // Return the created product
}

// Update product details (e.g., location, status, certification, IoT data)
#[ic_cdk::update]
fn update_product(id: u64, payload: ProductPayload) -> Result<Product, Error> {
    // Validate the product payload before updating
    validate_product_payload(&payload)?;

    match PRODUCT_STORAGE.with(|storage| storage.borrow().get(&id)) {
        Some(mut product) => {
            // Update product fields with new data
            product.current_location = payload.current_location;
            product.status = payload.status;
            product.certification = payload.certification;
            product.iot_data = payload.iot_data;
            product.last_update = Some(time());  // Update last modified timestamp
            do_insert(&product);  // Insert the updated product back into storage
            Ok(product)  // Return the updated product
        }
        None => Err(Error::NotFound {
            msg: format!("Cannot update product with id={}. Product not found", id),  // Return error if product not found
        }),
    }
}

// Delete a product entry by ID
#[ic_cdk::update]
fn delete_product(id: u64) -> Result<Product, Error> {
    match PRODUCT_STORAGE.with(|storage| storage.borrow_mut().remove(&id)) {
        Some(product) => Ok(product),  // Product found and deleted
        None => Err(Error::NotFound {
            msg: format!("Cannot delete product with id={}. Product not found.", id),  // Return error if product not found
        }),
    }
}

// Helper method for inserting a product into storage
fn do_insert(product: &Product) {
    PRODUCT_STORAGE.with(|storage| storage.borrow_mut().insert(product.id, product.clone()));  // Insert product into storage
}

// Helper method to retrieve a product by ID
fn _get_product(id: &u64) -> Option<Product> {
    PRODUCT_STORAGE.with(|storage| storage.borrow().get(id))  // Retrieve product from storage
}

// Custom error handling
#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },  // Product not found error
    InvalidInput { msg: String },  // Invalid input error
}

// Candid export for interface generation
ic_cdk::export_candid!();
