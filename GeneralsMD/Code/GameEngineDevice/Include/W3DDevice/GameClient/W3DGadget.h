// FILE: W3DGadget.h //////////////////////////////////////////////////////////
//
// Project:    RTS3
//
// File name:  W3DGadget.h
//
// Created:    Colin Day, June 2001
//
// Desc:       Implemtation details for various gadgets as they pertain to
//						 W3D will go here
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3DGADGET_H_
#define __W3DGADGET_H_

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////
#include "GameClient/Gadget.h"
#include "W3DDevice/GameClient/W3DGameWindow.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////

// TYPE DEFINES ///////////////////////////////////////////////////////////////
/// when drawing line art for gadgets, the borders are this size
#define WIN_DRAW_LINE_WIDTH (1.0f)

// INLINING ///////////////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// EXTERNALS //////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////

extern void W3DGadgetPushButtonDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetPushButtonImageDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetCheckBoxDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetCheckBoxImageDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetRadioButtonDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetRadioButtonImageDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetTabControlDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetTabControlImageDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetListBoxDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetListBoxImageDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetComboBoxDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetComboBoxImageDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetHorizontalSliderDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetHorizontalSliderImageDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetVerticalSliderDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetVerticalSliderImageDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetProgressBarDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetProgressBarImageDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetStaticTextDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetStaticTextImageDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetTextEntryDraw( GameWindow *window, WinInstanceData *instData );
extern void W3DGadgetTextEntryImageDraw( GameWindow *window, WinInstanceData *instData );

#endif // __W3DGADGET_H_

