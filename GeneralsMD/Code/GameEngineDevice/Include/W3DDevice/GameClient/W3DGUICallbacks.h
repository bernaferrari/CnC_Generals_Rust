// FILE: W3DGUICallbacks.h ////////////////////////////////////////////////////////////////////////
// Created:    Colin Day, August 2001
// Desc:       Callbacks for GUI elements that are specifically tied to
//						 a W3D implementation
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3DGUICALLBACKS_H_
#define __W3DGUICALLBACKS_H_

class GameWindow;
class WindowLayout;
class WinInstanceData;

// EXTERNALS //////////////////////////////////////////////////////////////////////////////////////

// Message of the day message window --------------------------------------------------------------
extern void W3DLeftHUDDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DCameoMovieDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DRightHUDDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DPowerDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DMainMenuDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DMainMenuFourDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DMetalBarMenuDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DCreditsMenuDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DClockDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DMainMenuMapBorder( GameWindow *window, WinInstanceData *instData );
extern void W3DMainMenuButtonDropShadowDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DMainMenuRandomTextDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DThinBorderDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DShellMenuSchemeDraw( GameWindow *window, WinInstanceData *instData );

extern void W3DCommandBarGridDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DCommandBarGenExpDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DCommandBarHelpPopupDraw( GameWindow *window, WinInstanceData *instData );

extern void W3DCommandBarBackgroundDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DCommandBarForegroundDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DCommandBarTopDraw( GameWindow *window, WinInstanceData *instData );

extern void W3DNoDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DDrawMapPreview( GameWindow *window, WinInstanceData *instData );

void W3DMainMenuInit( WindowLayout *layout, void *userData );

#endif // __W3DGUICALLBACKS_H_

