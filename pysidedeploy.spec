[app]

# title of your application
title = FDXAtelier

# project root directory. default = The parent directory of input_file
project_dir = .

# source file entry point path. default = main.py
input_file = main.py

# directory where the executable output is generated
exec_directory = dist

# path to the project file relative to project_dir
project_file = 

# application icon
icon = assets/fdx.ico

[python]

# python packages to install
packages = Nuitka==4.3.1

# buildozer = for deploying Android application
android_packages = buildozer==1.6.0,cython==0.29.33

[qt]

# paths to required qml files. comma separated
# normally all the qml files required by the project are added automatically
# design studio projects include the qml files using qt resources
qml_files = 

# excluded qml plugin binaries
excluded_qml_plugins = 

# qt modules used. comma separated
modules = Core,Gui,Widgets

# qt plugins used by the application. only relevant for desktop deployment
# for qt plugins used in android application see [android][plugins]
plugins = accessiblebridge,egldeviceintegrations,generic,iconengines,imageformats,platforminputcontexts,platforms,platforms/darwin,platformthemes,styles,wayland-decoration-client,wayland-graphics-integration-client,wayland-shell-integration,xcbglintegrations

[android]

# path to pyside wheel
wheel_pyside = 

# path to shiboken wheel
wheel_shiboken = 

# plugins to be copied to libs folder of the packaged application. comma separated
plugins = 

[ios]

# path to pyside wheel
wheel_pyside = 

# path to shiboken wheel
wheel_shiboken = 

# path to python.xcframework
xcframework_path = 

# reverse-dns bundle identifier, eg = com.example.myapp
# default (like xcode's own placeholder) = com.example.<app name, alphanumeric only>
bundle_id = 

# apple developer team id, used for code signing. only required for device builds
team_id = 

# app version (cfbundleshortversionstring)
version = 1.0

# minimum ios deployment target. default = read from the PySide6 wheel's embedded
# qt-for-ios kit
deployment_target = 

# path to an .entitlements file, relative to project_dir
entitlements = 

# code signing style. possible values = [Automatic, Manual]
signing_style = Automatic

# extra header search paths, relative to project_dir. comma separated
header_search_paths = 

[nuitka]

# usage description for permissions requested by the app as found in the info.plist file
# of the app bundle. comma separated
# eg = extra_args = --show-modules --follow-stdlib
macos.permissions = 

# mode of using nuitka. accepts standalone or onefile. default = standalone
# standalone creates a folder with the executable and its dependencies.
# onefile creates a single executable that unpacks itself at every start.
mode = standalone

# specify any extra nuitka arguments
extra_args = --assume-yes-for-downloads --mingw64 --experimental=force-dependencies-pefile --user-plugin=scripts/nuitka_pe_plugin.py --output-filename=FDXAtelier.exe --include-package=fdx_atelier --include-package=pandera --include-package=phonenumbers.data --nofollow-import-to=pytest,hypothesis,matplotlib,scipy,IPython,notebook,PyQt6,phonenumbers.geodata,phonenumbers.carrierdata,phonenumbers.tzdata --windows-console-mode=disable --jobs=4 --report=build/compilation-report.xml

[buildozer]

# build mode
# possible values = ["aarch64", "x86_64"]
# release creates a .aab, while debug creates a .apk
mode = debug

# path to pyside6 and shiboken6 recipe dir
recipe_dir = 

# path to extra qt android .jar files to be loaded by the application
jars_dir = 

# if empty, uses default ndk path downloaded by buildozer
ndk_path = 

# if empty, uses default sdk path downloaded by buildozer
sdk_path = 

# other libraries to be loaded at app startup. comma separated.
local_libs = 

# architecture of deployed platform
arch = 

