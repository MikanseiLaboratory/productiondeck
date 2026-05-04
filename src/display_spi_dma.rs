//! SPI bus wrapper so [`embedded_hal::spi::SpiBus`] writes use RP2040 DMA for bulk TX (TC-00 ST7789).

const MIN_DMA_TX_LEN: usize = 64;

use core::future::Future;
use core::pin::pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use cortex_m::asm::wfe;
use embassy_rp::spi::{Async, Instance, Spi};
use embedded_hal::spi::SpiBus;

unsafe fn waker_clone(p: *const ()) -> RawWaker {
    RawWaker::new(p, &FLAG_WAKER_VTABLE)
}
unsafe fn waker_wake(p: *const ()) {
    (*(p as *const AtomicBool)).store(true, Ordering::Release);
}
unsafe fn waker_wake_by_ref(p: *const ()) {
    waker_wake(p);
}
unsafe fn waker_drop(_: *const ()) {}

static FLAG_WAKER_VTABLE: RawWakerVTable =
    RawWakerVTable::new(waker_clone, waker_wake, waker_wake_by_ref, waker_drop);

fn waker_flag(flag: &AtomicBool) -> Waker {
    unsafe {
        Waker::from_raw(RawWaker::new(
            flag as *const _ as *const (),
            &FLAG_WAKER_VTABLE,
        ))
    }
}

fn block_on_dma_write<T: Instance>(
    spi: &mut Spi<'_, T, Async>,
    words: &[u8],
) -> Result<(), embassy_rp::spi::Error> {
    let done = AtomicBool::new(false);
    let waker = waker_flag(&done);
    let mut cx = Context::from_waker(&waker);
    let mut fut = pin!(spi.write(words));
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(r) => return r,
            Poll::Pending => loop {
                if done.load(Ordering::Acquire) {
                    done.store(false, Ordering::Release);
                    break;
                }
                wfe();
            },
        }
    }
}

/// SPI bus for ST7789: TX uses DMA for longer bursts.
pub struct DisplaySpiBus<'d, T: Instance>(pub Spi<'d, T, Async>);

impl<T: Instance> embedded_hal::spi::ErrorType for DisplaySpiBus<'_, T> {
    type Error = embassy_rp::spi::Error;
}

impl<T: Instance> SpiBus<u8> for DisplaySpiBus<'_, T> {
    fn flush(&mut self) -> Result<(), Self::Error> {
        self.0.flush()
    }

    fn read(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        self.0.blocking_read(words)
    }

    fn write(&mut self, words: &[u8]) -> Result<(), Self::Error> {
        if words.len() < MIN_DMA_TX_LEN {
            self.0.blocking_write(words)
        } else {
            block_on_dma_write(&mut self.0, words)
        }
    }

    fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Self::Error> {
        self.0.blocking_transfer(read, write)
    }

    fn transfer_in_place(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        self.0.blocking_transfer_in_place(words)
    }
}
