use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use super::models::Student;
use ruvoraq::prelude::*;

#[derive(Default)]
struct Store {
    next_id: u64,
    students: BTreeMap<u64, Student>,
}

// No Clone implementation: every handler shares this one service instance.
#[derive(Default)]
pub struct SchoolService {
    store: Mutex<Store>,
    visits: AtomicUsize,
}

impl SchoolService {
    pub fn visit(&self) -> usize {
        self.visits.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn create(&self, name: String) -> Result<Student> {
        let mut store = self.store.lock().map_err(|_| Error::internal())?;
        store.next_id = store.next_id.checked_add(1).ok_or_else(Error::internal)?;
        let student = Student {
            id: store.next_id,
            name: name.trim().to_owned(),
        };
        store.students.insert(student.id, student.clone());
        Ok(student)
    }

    pub fn list(&self, limit: usize, min_id: Option<u64>) -> Result<Vec<Student>> {
        let store = self.store.lock().map_err(|_| Error::internal())?;
        Ok(store
            .students
            .values()
            .filter(|student| min_id.is_none_or(|minimum| student.id >= minimum))
            .take(limit)
            .cloned()
            .collect())
    }

    pub async fn find(&self, id: u64) -> Result<Student> {
        let store = self.store.lock().map_err(|_| Error::internal())?;
        store
            .students
            .get(&id)
            .cloned()
            .ok_or_else(|| not_found("Student not found"))
    }

    pub fn rename(&self, id: u64, name: String) -> Result<Student> {
        let mut store = self.store.lock().map_err(|_| Error::internal())?;
        let student = store
            .students
            .get_mut(&id)
            .ok_or_else(|| not_found("Student not found"))?;
        student.name = name.trim().to_owned();
        Ok(student.clone())
    }

    pub fn delete(&self, id: u64) -> Result<()> {
        let mut store = self.store.lock().map_err(|_| Error::internal())?;
        store
            .students
            .remove(&id)
            .ok_or_else(|| not_found("Student not found"))?;
        Ok(())
    }
}
