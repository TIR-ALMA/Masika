use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core::hint::spin_loop;

const MAX_SPIN: usize = 64;

#[repr(align(64))]
pub struct Spinlock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
    owner_cpu: AtomicUsize,
    recursion_count: AtomicUsize,
}

unsafe impl<T: Send> Sync for Spinlock<T> {}

impl<T> Spinlock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
            owner_cpu: AtomicUsize::new(!0),
            recursion_count: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub fn try_lock(&self) -> Option<SpinlockGuard<T>> {
        if self.locked.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            Some(SpinlockGuard { lock: self })
        } else {
            None
        }
    }

    pub fn lock(&self) -> SpinlockGuard<T> {
        let mut spin_count = 0;
        loop {
            match self.locked.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed) {
                Ok(_) => break,
                Err(_) => {
                    spin_count += 1;
                    if spin_count < MAX_SPIN {
                        spin_loop();
                    } else {
                        spin_count = 0;
                        x86_64::instructions::interrupts::disable();
                        x86_64::instructions::hlt();
                        x86_64::instructions::interrupts::enable();
                    }
                }
            }
        }
        SpinlockGuard { lock: self }
    }

    fn raw_unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

#[repr(align(64))]
pub struct IrqSafeSpinlock<T> {
    inner: Spinlock<T>,
}

impl<T> IrqSafeSpinlock<T> {
    pub const fn new(data: T) -> Self {
        Self { inner: Spinlock::new(data) }
    }

    pub fn lock(&self) -> IrqSafeSpinlockGuard<T> {
        x86_64::instructions::interrupts::disable();
        let guard = self.inner.lock();
        IrqSafeSpinlockGuard { guard }
    }
}

pub struct IrqSafeSpinlockGuard<'a, T> {
    guard: SpinlockGuard<'a, T>,
}

impl<'a, T> Drop for IrqSafeSpinlockGuard<'a, T> {
    fn drop(&mut self) {
        drop(self.guard);
        x86_64::instructions::interrupts::enable();
    }
}

impl<'a, T> core::ops::Deref for IrqSafeSpinlockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &*self.guard
    }
}

impl<'a, T> core::ops::DerefMut for IrqSafeSpinlockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.guard
    }
}

pub struct SpinlockGuard<'a, T> {
    lock: &'a Spinlock<T>,
}

impl<'a, T> Drop for SpinlockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.raw_unlock();
    }
}

impl<'a, T> core::ops::Deref for SpinlockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> core::ops::DerefMut for SpinlockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

#[cfg(debug_assertions)]
mod debug {
    use super::*;
    use core::sync::atomic::{AtomicU64, Ordering};

    static LOCK_COUNTER: AtomicU64 = AtomicU64::new(0);

    impl<T> Spinlock<T> {
        pub fn debug_lock(&self) -> SpinlockGuard<T> {
            let id = LOCK_COUNTER.fetch_add(1, Ordering::SeqCst);
            log::debug!("Locking spinlock #{}", id);
            self.lock()
        }
    }
}

#[cfg(not(debug_assertions))]
mod release {
    use super::*;

    impl<T> Spinlock<T> {
        pub fn debug_lock(&self) -> SpinlockGuard<T> {
            self.lock()
        }
    }
}

impl<T> Spinlock<T> {
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Relaxed)
    }
}

pub struct TryLockError;

impl<T> Spinlock<T> {
    pub fn try_lock_or_yield(&self) -> Result<SpinlockGuard<T>, TryLockError> {
        match self.try_lock() {
            Some(guard) => Ok(guard),
            None => {
                spin_loop();
                Err(TryLockError)
            }
        }
    }
}

impl<T> Default for Spinlock<T>
where
    T: Default,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

pub fn spin_hint() {
    spin_loop();
}

#[derive(Debug)]
pub struct RawSpinlock {
    locked: AtomicBool,
}

impl RawSpinlock {
    pub const fn new() -> Self {
        Self { locked: AtomicBool::new(false) }
    }

    pub fn acquire(&self) {
        while self.locked.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            while self.locked.load(Ordering::Relaxed) {
                spin_loop();
            }
        }
    }

    pub fn release(&self) {
        self.locked.store(false, Ordering::Release);
    }

    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Relaxed)
    }
}

impl<T> Spinlock<T> {
    pub fn try_lock_for_debug(&self) -> Option<SpinlockGuard<T>> {
        self.try_lock()
    }

    pub fn force_unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

pub struct WeakSpinlockGuard<'a, T> {
    lock: &'a Spinlock<T>,
}

impl<'a, T> WeakSpinlockGuard<'a, T> {
    pub fn new(lock: &'a Spinlock<T>) -> Option<Self> {
        if lock.locked.load(Ordering::Relaxed) {
            Some(Self { lock })
        } else {
            None
        }
    }
}

impl<'a, T> Drop for WeakSpinlockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.raw_unlock();
    }
}

impl<'a, T> core::ops::Deref for WeakSpinlockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> core::ops::DerefMut for WeakSpinlockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

pub fn yield_spin() {
    for _ in 0..10 {
        spin_loop();
    }
}

pub struct AdaptiveSpinlock<T> {
    inner: Spinlock<T>,
}

impl<T> AdaptiveSpinlock<T> {
    pub const fn new(data: T) -> Self {
        Self { inner: Spinlock::new(data) }
    }

    pub fn lock(&self) -> SpinlockGuard<T> {
        self.inner.lock()
    }
}

pub fn pause_cpu() {
    x86_64::instructions::hints::pause();
}

pub fn atomic_bool_load_acquire(ptr: &AtomicBool) -> bool {
    ptr.load(Ordering::Acquire)
}

pub fn atomic_bool_store_release(ptr: &AtomicBool, val: bool) {
    ptr.store(val, Ordering::Release);
}

pub fn compare_and_swap_acquire_release(
    ptr: &AtomicBool,
    current: bool,
    new: bool,
) -> Result<bool, bool> {
    match ptr.compare_exchange(current, new, Ordering::AcqRel, Ordering::Relaxed) {
        Ok(v) => Ok(v),
        Err(v) => Err(v),
    }
}

pub fn busy_wait() {
    for _ in 0..100 {
        spin_loop();
    }
}

pub fn cpu_relax() {
    spin_loop();
}

pub fn acquire_with_backoff<F>(mut f: F) -> bool
where
    F: FnMut() -> bool,
{
    let mut count = 0;
    loop {
        if f() {
            return true;
        }
        if count > 100 {
            x86_64::instructions::hlt();
            count = 0;
        } else {
            count += 1;
            spin_loop();
        }
    }
}

pub fn try_acquire<F>(f: F) -> bool
where
    F: FnOnce() -> bool,
{
    f()
}

pub fn release_fence() {
    core::sync::atomic::fence(Ordering::Release);
}

pub fn acquire_fence() {
    core::sync::atomic::fence(Ordering::Acquire);
}

pub fn acquire_release_fence() {
    core::sync::atomic::fence(Ordering::AcqRel);
}

pub fn relaxed_fence() {
    core::sync::atomic::fence(Ordering::Relaxed);
}

pub fn seq_cst_fence() {
    core::sync::atomic::fence(Ordering::SeqCst);
}

pub fn atomic_usize_load_acquire(ptr: &AtomicUsize) -> usize {
    ptr.load(Ordering::Acquire)
}

pub fn atomic_usize_store_release(ptr: &AtomicUsize, val: usize) {
    ptr.store(val, Ordering::Release);
}

pub fn compare_and_swap_usize_acquire_release(
    ptr: &AtomicUsize,
    current: usize,
    new: usize,
) -> Result<usize, usize> {
    match ptr.compare_exchange(current, new, Ordering::AcqRel, Ordering::Relaxed) {
        Ok(v) => Ok(v),
        Err(v) => Err(v),
    }
}

pub fn yield_current_thread() {
    for _ in 0..50 {
        spin_loop();
    }
}

pub fn wait_until<F>(mut condition: F)
where
    F: FnMut() -> bool,
{
    while !condition() {
        spin_loop();
    }
}

pub fn wait_until_timeout<F>(mut condition: F, timeout: usize) -> bool
where
    F: FnMut() -> bool,
{
    let mut count = 0;
    while !condition() {
        if count >= timeout {
            return false;
        }
        count += 1;
        spin_loop();
    }
    true
}

pub fn atomic_bool_swap_acquire_release(ptr: &AtomicBool, val: bool) -> bool {
    ptr.swap(val, Ordering::AcqRel)
}

pub fn atomic_usize_swap_acquire_release(ptr: &AtomicUsize, val: usize) -> usize {
    ptr.swap(val, Ordering::AcqRel)
}

pub fn spin_on<F>(mut condition: F)
where
    F: FnMut() -> bool,
{
    loop {
        if condition() {
            break;
        }
        spin_loop();
    }
}

pub fn exponential_backoff<F>(mut f: F) -> bool
where
    F: FnMut() -> bool,
{
    let mut delay = 1;
    loop {
        if f() {
            return true;
        }
        for _ in 0..delay {
            spin_loop();
        }
        delay *= 2;
        if delay > 1024 {
            delay = 1024;
        }
    }
}

pub fn acquire_with_hint<F>(mut f: F) -> bool
where
    F: FnMut() -> bool,
{
    loop {
        if f() {
            return true;
        }
        x86_64::instructions::hints::pause();
    }
}

