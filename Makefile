PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
DATADIR ?= $(PREFIX)/share
APPDIR ?= $(DATADIR)/applications
PRESETSDIR ?= $(DATADIR)/clickclick/presets

.PHONY: all build install uninstall clean

all: build

build:
	cargo build --release

install: build
	install -Dm755 target/release/clickclickd $(DESTDIR)$(BINDIR)/clickclickd
	install -Dm755 bin/clickclick $(DESTDIR)$(BINDIR)/clickclick
	ln -sf clickclick $(DESTDIR)$(BINDIR)/clickclick-ctl
	ln -sf clickclick $(DESTDIR)$(BINDIR)/clickclick-cli
	install -Dm755 ui/clickclick_gui.py $(DESTDIR)$(BINDIR)/clickclick-settings
	install -Dm644 ui/com.github.saschlik.ClickClick.desktop $(DESTDIR)$(APPDIR)/com.github.saschlik.ClickClick.desktop
	mkdir -p $(DESTDIR)$(PRESETSDIR)
	cp -r presets/* $(DESTDIR)$(PRESETSDIR)/

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/clickclickd
	rm -f $(DESTDIR)$(BINDIR)/clickclick
	rm -f $(DESTDIR)$(BINDIR)/clickclick-ctl
	rm -f $(DESTDIR)$(BINDIR)/clickclick-cli
	rm -f $(DESTDIR)$(BINDIR)/clickclick-settings
	rm -f $(DESTDIR)$(APPDIR)/com.github.saschlik.ClickClick.desktop
	rm -rf $(DESTDIR)$(DATADIR)/clickclick

clean:
	cargo clean
