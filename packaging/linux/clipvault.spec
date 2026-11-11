Name:           clipvault
Version:        %{clipvault_version}
Release:        1%{?dist}
Summary:        Private native Linux clipboard history
License:        MIT
URL:            https://github.com/Amibiplob/ClipVault
BuildArch:      x86_64
Requires:       glibc >= 2.39, xkeyboard-config
Requires:       libsqlite3.so.0()(64bit), libgdk_pixbuf-2.0.so.0()(64bit)
Requires:       libX11.so.6()(64bit), libX11-xcb.so.1()(64bit), libXcursor.so.1()(64bit), libXi.so.6()(64bit), libXrandr.so.2()(64bit), libXinerama.so.1()(64bit), libXrender.so.1()(64bit), libXfixes.so.3()(64bit), libXext.so.6()(64bit)
Requires:       libxkbcommon.so.0()(64bit), libxkbcommon-x11.so.0()(64bit), libxcb.so.1()(64bit)
Requires:       libwayland-client.so.0()(64bit), libwayland-cursor.so.0()(64bit), libwayland-egl.so.1()(64bit), libGL.so.1()(64bit), libEGL.so.1()(64bit)
Recommends:     fontconfig
AutoReqProv:    yes
%description
Local clipboard history with text, supported rich formats, images and file
references. No cloud sync. Desktop capabilities and testing vary by session.
%prep
%build
%install
mkdir -p %{buildroot}
cp -a %{clipvault_stage}/usr %{buildroot}/
%post
if command -v update-desktop-database >/dev/null; then update-desktop-database -q /usr/share/applications || :; fi
if command -v gtk-update-icon-cache >/dev/null; then gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || :; fi
%postun
if command -v update-desktop-database >/dev/null; then update-desktop-database -q /usr/share/applications || :; fi
if command -v gtk-update-icon-cache >/dev/null; then gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || :; fi
%files
%license /usr/share/doc/clipvault/LICENSE
/usr/bin/clipvault
/usr/share/applications/io.clipvault.desktop.desktop
/usr/share/metainfo/io.clipvault.desktop.metainfo.xml
/usr/share/icons/hicolor/*/apps/clipvault.*
/usr/share/doc/clipvault/*
