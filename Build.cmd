@echo off
rem RustCraft: fordit, es frissiti a jatekot a dist\RustCraft mappaban.
rem A mappat (RustCraft.exe + resourcepacks) egyben lehet becsomagolni es elkuldeni.
setlocal
cd /d "%~dp0"
title RustCraft - Build
set "GAME=dist\RustCraft"

:running
tasklist /fi "imagename eq RustCraft.exe" 2>nul | find /i "RustCraft.exe" >nul
if not errorlevel 1 (
    echo A jatek meg fut. Zard be, aztan nyomj meg egy gombot...
    pause >nul
    goto running
)

rem Ha a cargo nincs a PATH-ban (pl. friss telepites utan nem nyitottal uj ablakot),
rem megnezzuk a rustup alapertelmezett helyen is.
where cargo >nul 2>nul
if errorlevel 1 if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where cargo >nul 2>nul
if errorlevel 1 (
    echo.
    echo ============================================================
    echo  HIBA: nincs telepitve a Rust ^(cargo nem talalhato^).
    echo.
    echo  1. Toltsd le es futtasd: https://rustup.rs  ^(rustup-init.exe^)
    echo     - ha Visual Studio Build Tools-t ker, fogadd el
    echo  2. Utana zard be ezt az ablakot, es inditsd ujra a Build.cmd-t.
    echo ============================================================
    goto end
)

echo Forditas...
cargo build --release
if errorlevel 1 (
    echo.
    echo ============================================================
    echo  HIBA: nem sikerult leforditani. A hiba fent olvashato.
    echo ============================================================
    goto end
)

if not exist "%GAME%\resourcepacks" mkdir "%GAME%\resourcepacks"
copy /y "target\release\rustcraft.exe" "%GAME%\RustCraft.exe" >nul
if errorlevel 1 (
    echo.
    echo HIBA: nem sikerult bemasolni az exe-t ide: %GAME%
    goto end
)
copy /y "CREDITS.md" "%GAME%\CREDITS.md" >nul

echo.
echo ============================================================
echo  KESZ: %GAME%\RustCraft.exe frissitve.
echo ============================================================

:end
echo.
pause
