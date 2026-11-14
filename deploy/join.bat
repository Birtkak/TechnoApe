@echo off
rem TechnoApe's launcher (ADR 0130): takes a newer build from the server when there is one, then joins it.
cd /d "%~dp0"
set URL=http://100.124.84.125:7780
curl -fs -m 5 %URL%/version.txt -o version.new || goto play
fc /b version.new version.txt >nul 2>&1 && goto play
echo Updating TechnoApe...
curl -f -m 900 %URL%/TechnoApe.zip -o update.zip || goto play
rem one block: cmd has read all of it before tar replaces this file
(
  rmdir /s /q data 2>nul
  tar -xf update.zip --strip-components 1 || (echo The update failed: close the game and start this again. & pause & exit /b 1)
  del update.zip version.new 2>nul
  start "" TechnoApe.exe run=play onscreen=1 connect=100.124.84.125:7777 name=%USERNAME% %*
  exit /b
)
:play
del version.new 2>nul
start "" TechnoApe.exe run=play onscreen=1 connect=100.124.84.125:7777 name=%USERNAME% %*
