#!/usr/bin/env python3
"""
ClickClick Settings GUI (Standalone GNOME / Libadwaita App)
Konfiguration für die mechanische Tastatur-Akustik-Engine von BalonixOS.
"""
import os
import sys
import subprocess
import glob
import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, GLib, Gio

HOME = os.path.expanduser("~")
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
CLICKCLICK_SH = os.path.join(SCRIPT_DIR, "clickclick.sh")
CONFIG_DIR = os.path.join(HOME, ".config/clickclick")
PRESETS_DIR = os.path.join(CONFIG_DIR, "presets")


class ClickClickApp(Adw.Application):
    def __init__(self):
        super().__init__(
            application_id="org.balonix.ClickClick",
            flags=Gio.ApplicationFlags.FLAGS_NONE
        )

    def do_activate(self):
        win = self.props.active_window
        if not win:
            win = ClickClickWindow(application=self)
        win.present()


class ClickClickWindow(Adw.ApplicationWindow):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.set_title("ClickClick Tastatur-Akustik")
        self.set_default_size(680, 720)
        self._updating = False

        # Main Layout
        content_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)

        # HeaderBar
        header = Adw.HeaderBar()
        title_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        title_box.set_valign(Gtk.Align.CENTER)
        h_title = Gtk.Label(label="ClickClick", css_classes=["title"])
        h_sub = Gtk.Label(label="Mechanische Tastatur-Akustik", css_classes=["subtitle", "caption"])
        title_box.append(h_title)
        title_box.append(h_sub)
        header.set_title_widget(title_box)

        # Test Click Button in HeaderBar
        self.test_btn = Gtk.Button(label="Test-Klick 🎵")
        self.test_btn.set_tooltip_text("Spielt ein Klicksample des aktiven Schalters ab")
        self.test_btn.connect("clicked", self.on_play_test_click)
        header.pack_end(self.test_btn)

        content_box.append(header)

        # Scrolled Preferences Page
        scroll = Gtk.ScrolledWindow(vexpand=True, hexpand=True)
        scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        page = Adw.PreferencesPage()

        # ── Group 1: Engine Status & Master Switch ────────────────────────────
        group_master = Adw.PreferencesGroup()
        group_master.set_title("Engine Status")
        group_master.set_description("Hintergrund-Daemon für ultra-latenzfreies akustisches Feedback (0 ms).")

        self.master_switch = Adw.SwitchRow()
        self.master_switch.set_title("Akustisches Tasten-Feedback")
        self.master_switch.set_subtitle("Mechanische Klicksounds bei jedem Tastenanschlag abspielen")
        self.master_switch.connect("notify::active", self.on_toggle_master)
        group_master.add(self.master_switch)

        self.status_row = Adw.ActionRow()
        self.status_row.set_title("Zustand")
        self.status_badge = Gtk.Label(label="Wird geladen...", css_classes=["caption", "dim-label"])
        self.status_row.add_suffix(self.status_badge)
        group_master.add(self.status_row)

        page.add(group_master)

        # ── Group 2: Schalter-Profil & Akustik ─────────────────────────────────
        group_switch = Adw.PreferencesGroup()
        group_switch.set_title("Schalter-Charakter &amp; Akustik")
        group_switch.set_description("Wähle das mechanische Schalterprofil und passe die Lautstärke an.")

        self.preset_row = Adw.ComboRow()
        self.preset_row.set_title("Mechanischer Switch (Preset)")
        self.preset_row.set_subtitle("Klangcharakter der Tastenkappen und Schalterfedern")
        self.presets_data = [
            ("NK_Cream (Linear Smooth)", "nk_cream"),
            ("Cherry MX Black PBT (Deep Clack)", "cherry_mx_black_pbt"),
            ("Unicomp Buckling Spring (Klassisch)", "unicomp_classic"),
            ("Default Clicky (Hell & Knackig)", "default_clicky"),
        ]
        model_p = Gtk.StringList.new([name for name, _ in self.presets_data])
        self.preset_row.set_model(model_p)
        self.preset_row.connect("notify::selected", self.on_preset_selected)
        group_switch.add(self.preset_row)

        self.vol_row = Adw.ActionRow()
        self.vol_row.set_title("Klick-Lautstärke")
        self.vol_row.set_subtitle("Lautstärke der mechanischen Klicks")
        self.vol_scale = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 100, 1)
        self.vol_scale.set_size_request(220, -1)
        self.vol_scale.set_valign(Gtk.Align.CENTER)
        self.vol_scale.set_draw_value(True)
        self.vol_scale.set_value_pos(Gtk.PositionType.RIGHT)
        self.vol_scale.connect("value-changed", self.on_vol_changed)
        self.vol_row.add_suffix(self.vol_scale)
        group_switch.add(self.vol_row)

        self.hum_switch = Adw.SwitchRow()
        self.hum_switch.set_title("Organische Humanisierung")
        self.hum_switch.set_subtitle("Zufällige Mikro-Variationen in Tonhöhe und Dynamik (verhindert Maschinengewehr-Effekt)")
        self.hum_switch.connect("notify::active", self.on_hum_changed)
        group_switch.add(self.hum_switch)

        self.mice_switch = Adw.SwitchRow()
        self.mice_switch.set_title("Mausklick-Akustik")
        self.mice_switch.set_subtitle("Sanftes Feedback auch bei Maustasten (Razer Basilisk / Standardmaus)")
        self.mice_switch.connect("notify::active", self.on_mice_changed)
        group_switch.add(self.mice_switch)

        page.add(group_switch)

        # ── Group 3: Audio-Routing & Latenz ───────────────────────────────────
        group_routing = Adw.PreferencesGroup()
        group_routing.set_title("Audio-Routing &amp; Latenz")
        group_routing.set_description("Optimiert für PipeWire: Wähle zwischen direktem Hardware-DAC oder 3D-Filterkette.")

        self.routing_row = Adw.ComboRow()
        self.routing_row.set_title("Audio-Pfad")
        self.routing_row.set_subtitle("Direkter DAC-Weg garantiert 0 ms Eingabeverzögerung")
        self.routing_modes = [
            ("Direkt (Hardware-Kopfhörer DAC, 0 Latenz)", "direct"),
            ("3D HRTF Raumklang (HeSuVi Filterkette)", "surround"),
            ("System-Standard (Standard-Ausgabe)", "default"),
        ]
        model_r = Gtk.StringList.new([name for name, _ in self.routing_modes])
        self.routing_row.set_model(model_r)
        self.routing_row.connect("notify::selected", self.on_routing_selected)
        group_routing.add(self.routing_row)

        self.sink_info_row = Adw.ActionRow()
        self.sink_info_row.set_title("Aktive Audio-Senke")
        self.sink_info_label = Gtk.Label(label="—", css_classes=["caption", "dim-label"])
        self.sink_info_row.add_suffix(self.sink_info_label)
        group_routing.add(self.sink_info_row)

        page.add(group_routing)

        # ── Group 4: Hardware & Hotplug ───────────────────────────────────────
        group_hw = Adw.PreferencesGroup()
        group_hw.set_title("Hardware &amp; Hotplug")
        group_hw.set_description("Vollständige Hardware-Unabhängigkeit: Geräte werden automatisch dynamisch eingebunden.")

        self.dev_row = Adw.ActionRow()
        self.dev_row.set_title("Erkannte Tastaturen")
        self.dev_row.set_subtitle("Hotplug aktiv – Tastaturwechsel (USB / Funk / Bluetooth / TMR) erfolgt sofort")
        self.dev_label = Gtk.Label(label="Erkenne...", css_classes=["caption", "dim-label"])
        self.dev_row.add_suffix(self.dev_label)
        group_hw.add(self.dev_row)

        page.add(group_hw)

        scroll.set_child(page)
        content_box.append(scroll)
        self.set_content(content_box)

        # Load initial state
        self.refresh_state()
        GLib.timeout_add_seconds(1, self.periodic_check)

    def periodic_check(self):
        if not self._updating:
            self.refresh_state()
        return True

    def run_cmd(self, cmd_args):
        try:
            return subprocess.run(cmd_args, capture_output=True, text=True, timeout=5).stdout.strip()
        except Exception:
            return ""

    def on_play_test_click(self, btn):
        active_wav = os.path.join(CONFIG_DIR, "active/space.wav")
        if not os.path.exists(active_wav):
            active_wav = os.path.join(CONFIG_DIR, "active/enter.wav")
        if not os.path.exists(active_wav):
            wavs = glob.glob(os.path.join(CONFIG_DIR, "active/*.wav"))
            if wavs:
                active_wav = wavs[0]

        if os.path.exists(active_wav):
            subprocess.Popen(["pw-play", active_wav])

    def refresh_state(self):
        self._updating = True
        try:
            # Running state
            pid = self.run_cmd(["pgrep", "-x", "clickclick_rs"])
            is_running = bool(pid)
            self.master_switch.set_active(is_running)
            if is_running:
                self.status_badge.set_text(f"Aktiv (PID {pid})")
                self.status_badge.remove_css_class("error")
                self.status_badge.add_css_class("success")
            else:
                self.status_badge.set_text("Gestoppt")
                self.status_badge.remove_css_class("success")
                self.status_badge.add_css_class("error")

            # Preset
            cur_preset = "nk_cream"
            preset_file = os.path.join(CONFIG_DIR, "active_preset.txt")
            if os.path.exists(preset_file):
                with open(preset_file, "r") as f:
                    cur_preset = f.read().strip()
            for i, (_, code) in enumerate(self.presets_data):
                if code == cur_preset:
                    self.preset_row.set_selected(i)
                    break

            # Volume
            vol_val = 75
            vol_file = os.path.join(CONFIG_DIR, "volume.txt")
            if os.path.exists(vol_file):
                try:
                    with open(vol_file, "r") as f:
                        vol_val = int(round(float(f.read().strip()) * 100))
                except Exception:
                    pass
            self.vol_scale.set_value(vol_val)

            # Humanize
            hum_val = True
            hum_file = os.path.join(CONFIG_DIR, "humanize.txt")
            if os.path.exists(hum_file):
                with open(hum_file, "r") as f:
                    hum_val = (f.read().strip().lower() == "true")
            self.hum_switch.set_active(hum_val)

            # Mice
            mice_val = True
            mice_file = os.path.join(CONFIG_DIR, "enable_mice.txt")
            if os.path.exists(mice_file):
                with open(mice_file, "r") as f:
                    mice_val = (f.read().strip().lower() == "true")
            self.mice_switch.set_active(mice_val)

            # Sink Mode
            sink_val = "direct"
            sink_file = os.path.join(CONFIG_DIR, "sink_mode.txt")
            if os.path.exists(sink_file):
                with open(sink_file, "r") as f:
                    sink_val = f.read().strip().lower()
            for i, (_, code) in enumerate(self.routing_modes):
                if code == sink_val:
                    self.routing_row.set_selected(i)
                    break

            # Read log to find connected hardware
            log_file = "/tmp/clickclick.log"
            devices = []
            if os.path.exists(log_file):
                try:
                    with open(log_file, "r") as f:
                        for line in f:
                            if "[+] Connected:" in line:
                                dev = line.split("[+] Connected:")[1].strip()
                                if dev and dev not in devices:
                                    devices.append(dev)
                except Exception:
                    pass
            if devices:
                keyb_devs = [d for d in devices if "keyboard" in d.lower()]
                if keyb_devs:
                    self.dev_label.set_text(", ".join(keyb_devs[:2]))
                else:
                    self.dev_label.set_text(devices[0])
            else:
                self.dev_label.set_text("Bereit (Hotplug Scan)")

            # Active sink name
            hw_sink = self.run_cmd(["bash", "-c", f"{CLICKCLICK_SH} status | grep 'Audio-Routing' || true"])
            self.sink_info_label.set_text(sink_val.capitalize())
        finally:
            self._updating = False

    def on_toggle_master(self, row, param):
        if self._updating:
            return
        active = row.get_active()
        cmd = [CLICKCLICK_SH, "start" if active else "stop"]
        subprocess.run(cmd)
        GLib.timeout_add(300, self.refresh_state)

    def on_preset_selected(self, row, param):
        if self._updating:
            return
        idx = row.get_selected()
        if 0 <= idx < len(self.presets_data):
            code = self.presets_data[idx][1]
            subprocess.run([CLICKCLICK_SH, "preset", code])
            GLib.timeout_add(200, self.refresh_state)

    def on_vol_changed(self, scale):
        if self._updating:
            return
        val = int(round(scale.get_value()))
        subprocess.run([CLICKCLICK_SH, "volume", str(val)])

    def on_hum_changed(self, row, param):
        if self._updating:
            return
        active = row.get_active()
        subprocess.run([CLICKCLICK_SH, "humanize", "on" if active else "off"])

    def on_mice_changed(self, row, param):
        if self._updating:
            return
        active = row.get_active()
        with open(os.path.join(CONFIG_DIR, "enable_mice.txt"), "w") as f:
            f.write("true" if active else "false")
        if bool(self.run_cmd(["pgrep", "-x", "clickclick_rs"])):
            subprocess.run([CLICKCLICK_SH, "start"])
        GLib.timeout_add(200, self.refresh_state)

    def on_routing_selected(self, row, param):
        if self._updating:
            return
        idx = row.get_selected()
        if 0 <= idx < len(self.routing_modes):
            code = self.routing_modes[idx][1]
            subprocess.run([CLICKCLICK_SH, "sink", code])
            GLib.timeout_add(200, self.refresh_state)


def main():
    app = ClickClickApp()
    return app.run(sys.argv)


if __name__ == "__main__":
    sys.exit(main())
