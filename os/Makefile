# Building
TARGET := riscv64gc-unknown-none-elf
MODE := release
KERNEL_ELF := target/$(TARGET)/$(MODE)/os
KERNEL_BIN := $(KERNEL_ELF).bin

# Board
BOARD := qemu
SBI ?= rustsbi
BOOTLOADER := ../bootloader/$(SBI)-$(BOARD).bin

# GUI
GUI ?= off
ifeq ($(GUI), off)
	GUI_OPTION := -display none
endif

# Building mode
ifeq ($(MODE), release)
	MODE_ARG := --release
endif

# KERNEL ENTRY
KERNEL_ENTRY_PA := 0x80200000

# Binutils
OBJDUMP := rust-objdump --arch-name=riscv64
OBJCOPY := rust-objcopy --binary-architecture=riscv64

# Disassembly
DISASM ?= -x

# Run usertests or usershell
TEST ?=

QEMU_ARGS := -machine virt \
			$(GUI_OPTION) \
			-nographic \
			-bios $(BOOTLOADER) \
			-device loader,file=$(KERNEL_BIN),addr=$(KERNEL_ENTRY_PA)

build: $(KERNEL_BIN)

run: build
	@qemu-system-riscv64 $(QEMU_ARGS)

$(KERNEL_BIN): kernel
	@$(OBJCOPY) $(KERNEL_ELF) --strip-all -O binary $@

kernel:
	@echo Platform: $(BOARD)
	@cp src/linker-$(BOARD).ld src/linker.ld
	cargo build --release
	@rm src/linker.ld

clean:
	cargo clean

disasm: kernel
	@$(OBJDUMP) $(DISASM) $(KERNEL_ELF)
