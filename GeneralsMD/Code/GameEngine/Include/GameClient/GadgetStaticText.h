// FILE: GadgetStaticText.h ///////////////////////////////////////////////////
//
// Project:    RTS3
//
// File name:  GadgetStaticText.h
//
// Created:    Colin Day, June 2001
//
// Desc:       Helpful interface for StaticTexts
//
// StaticText IMAGE/COLOR organization 
// When control is enabled:
//   enabledDrawData[ 0 ] is the background image for the whole enabled control
//
// When control is disabled:
//   disabledDrawData[ 0 ] is the background image for the whole disabled control
//
// When control is hilited (mouse over it and enabled)
//   hiliteDrawData[ 0 ] is the background image for the whole hilited control
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __GADGETSTATICTEXT_H_
#define __GADGETSTATICTEXT_H_

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////
#include "GameClient/GameWindow.h"
// FORWARD REFERENCES /////////////////////////////////////////////////////////

// TYPE DEFINES ///////////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// INLINING ///////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////

extern void GadgetStaticTextSetText( GameWindow *window, UnicodeString text );
extern UnicodeString GadgetStaticTextGetText( GameWindow *window );
extern void GadgetStaticTextSetFont( GameWindow *window, GameFont *font );

// text colors

// enabled background
inline void GadgetStaticTextSetEnabledImage( GameWindow *g, const Image *image )		{ g->winSetEnabledImage( 0, image ); }
inline void GadgetStaticTextSetEnabledColor( GameWindow *g, Color color )				{ g->winSetEnabledColor( 0, color ); }
inline void GadgetStaticTextSetEnabledBorderColor( GameWindow *g, Color color )	{ g->winSetEnabledBorderColor( 0, color ); }
inline const Image *GadgetStaticTextGetEnabledImage( GameWindow *g )								{ return g->winGetEnabledImage( 0 ); }
inline Color		 GadgetStaticTextGetEnabledColor( GameWindow *g )								{ return g->winGetEnabledColor( 0 ); }
inline Color		 GadgetStaticTextGetEnabledBorderColor( GameWindow *g )					{ return g->winGetEnabledBorderColor( 0 ); }

// disabled background
inline void GadgetStaticTextSetDisabledImage( GameWindow *g, const Image *image )		{ g->winSetDisabledImage( 0, image ); }
inline void GadgetStaticTextSetDisabledColor( GameWindow *g, Color color )				{ g->winSetDisabledColor( 0, color ); }
inline void GadgetStaticTextSetDisabledBorderColor( GameWindow *g, Color color )	{ g->winSetDisabledBorderColor( 0, color ); }
inline const Image *GadgetStaticTextGetDisabledImage( GameWindow *g )								{ return g->winGetDisabledImage( 0 ); }
inline Color		 GadgetStaticTextGetDisabledColor( GameWindow *g )								{ return g->winGetDisabledColor( 0 ); }
inline Color		 GadgetStaticTextGetDisabledBorderColor( GameWindow *g )					{ return g->winGetDisabledBorderColor( 0 ); }

// hilite if we choose to use it
inline void GadgetStaticTextSetHiliteImage( GameWindow *g, const Image *image )		{ g->winSetHiliteImage( 0, image ); }
inline void GadgetStaticTextSetHiliteColor( GameWindow *g, Color color )				{ g->winSetHiliteColor( 0, color ); }
inline void GadgetStaticTextSetHiliteBorderColor( GameWindow *g, Color color )	{ g->winSetHiliteBorderColor( 0, color ); }
inline const Image *GadgetStaticTextGetHiliteImage( GameWindow *g )								{ return g->winGetHiliteImage( 0 ); }
inline Color		 GadgetStaticTextGetHiliteColor( GameWindow *g )								{ return g->winGetHiliteColor( 0 ); }
inline Color		 GadgetStaticTextGetHiliteBorderColor( GameWindow *g )					{ return g->winGetHiliteBorderColor( 0 ); }

// EXTERNALS //////////////////////////////////////////////////////////////////

#endif // __GADGETSTATICTEXT_H_

