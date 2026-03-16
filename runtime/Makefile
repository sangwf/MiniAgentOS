CARGO := cargo +nightly
CARGO_FLAGS := -Z build-std=core,alloc -Z json-target-spec
TARGET := aarch64-virt
KERNEL := target/$(TARGET)/debug/minios
DTB := target/virt.dtb
QEMU_MMIO_DEVICE_SIZE ?=
QEMU_MMIO_DEVICE_SIZE_FLAG := $(if $(QEMU_MMIO_DEVICE_SIZE),-global virtio-mmio.devicesize=$(QEMU_MMIO_DEVICE_SIZE),)

build:
	$(CARGO) build $(CARGO_FLAGS)

run: build
	qemu-system-aarch64 -machine virt -cpu cortex-a53 -nographic -kernel $(KERNEL)

dtb:
	qemu-system-aarch64 -machine virt,dumpdtb=$(DTB) -display none -monitor none -S

run-dtb: build dtb
	qemu-system-aarch64 -machine virt -cpu cortex-a53 -nographic -m 512M -kernel $(KERNEL) -device loader,file=$(DTB),addr=0x40000000

run-net: build
	qemu-system-aarch64 -machine virt -cpu cortex-a53 -nographic -kernel $(KERNEL) \
		-global virtio-mmio.force-legacy=off \
		$(QEMU_MMIO_DEVICE_SIZE_FLAG) \
		-device virtio-net-device,x-disable-legacy-check=on,netdev=net0 -netdev user,id=net0

run-net-legacy: build
	qemu-system-aarch64 -machine virt -cpu cortex-a53 -nographic -kernel $(KERNEL) \
		-global virtio-mmio.force-legacy=on \
		$(QEMU_MMIO_DEVICE_SIZE_FLAG) \
		-device virtio-net-device,netdev=net0 -netdev user,id=net0

run-net-legacy-fwd: build
	qemu-system-aarch64 -machine virt -cpu cortex-a53 -nographic -kernel $(KERNEL) \
		-global virtio-mmio.force-legacy=on \
		$(QEMU_MMIO_DEVICE_SIZE_FLAG) \
		-device virtio-net-device,netdev=net0 -netdev user,id=net0,hostfwd=udp::5555-:5555

clean:
	$(CARGO) clean
