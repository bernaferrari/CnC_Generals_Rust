// FILE: GameWindowGlobal.cpp /////////////////////////////////////////////////
//
// Project:   RTS3
//
// File name: GameWindowGlobal.cpp
//
// Created:   Colin Day, June 2001
//
// Desc:      These are some global functions that every game using this
//						window system must implement for their current technology
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

// USER INCLUDES //////////////////////////////////////////////////////////////
#include "Common/Language.h"
#include "GameClient/Image.h"
#include "GameClient/Display.h"
#include "GameClient/GameWindowManager.h"
#include "GameClient/GameFont.h"

// DEFINES ////////////////////////////////////////////////////////////////////

// PRIVATE TYPES //////////////////////////////////////////////////////////////

// PRIVATE DATA ///////////////////////////////////////////////////////////////

// PUBLIC DATA ////////////////////////////////////////////////////////////////

// PRIVATE PROTOTYPES /////////////////////////////////////////////////////////

// PRIVATE FUNCTIONS //////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////

// GameWindowManager::winDrawImage ============================================
/** draw image, coord are in screen and should be kepth within 
	* that box specified */
//=============================================================================
void GameWindowManager::winDrawImage( const Image *image, Int startX, Int startY, 
																			Int endX, Int endY, Color color )
{

	TheDisplay->drawImage( image, startX, startY, endX, endY, color );

}  // end WinDrawImage

// GameWindowManager::winFillRect =============================================
/** draw filled rect, coords are absolute screen coords */
//=============================================================================
void GameWindowManager::winFillRect( Color color, Real width, 
																		 Int startX, Int startY, 
																		 Int endX, Int endY )
{

	TheDisplay->drawFillRect( startX, startY, 
														endX - startX, endY - startY, 
														color );

}  // end WinFillRect

// GameWindowManager::winOpenRect =============================================
/** draw rect outline, coords are absolute screen coords */
//=============================================================================
void GameWindowManager::winOpenRect( Color color, Real width, 
																		 Int startX, Int startY, 
																		 Int endX, Int endY )
{

	TheDisplay->drawOpenRect( startX, startY,
														endX - startX, endY - startY,
														width, color );

}  // end WinOpenRect

// GameWindowManager::winDrawLine =============================================
/** draw line, coords are absolute screen coords */
//=============================================================================
void GameWindowManager::winDrawLine( Color color, Real width, 
																		 Int startX, Int startY, 
																		 Int endX, Int endY )
{

	TheDisplay->drawLine( startX, startY, endX, endY, width, color );

}  // end WinDrawLine

// GameWindowManager::winFindImage ============================================
/** Given an image name, return an image loc to that image information.
	* You may reorganize the image loc structure however you like to suit 
	* your individual project */
//=============================================================================
const Image *GameWindowManager::winFindImage( const char *name ) 
{ 

	assert( TheMappedImageCollection );
	if( TheMappedImageCollection )
		return TheMappedImageCollection->findImageByName( AsciiString( name ) );

	return NULL;

}  // end WinFindImage

// GameWindowManager::winMakeColor ============================================
/** Given RGBA, make a color, you can change color representation for your
	* individual project needs */
//=============================================================================
Color GameWindowManager::winMakeColor( UnsignedByte red, 
																			 UnsignedByte green, 
																			 UnsignedByte blue, 
																			 UnsignedByte alpha ) 
{ 

	return GameMakeColor( red, green, blue, alpha );

}  // end WinMakeColor

// GameWindowManager::winFormatText ===========================================
/** draw text to the screen */
//=============================================================================
void GameWindowManager::winFormatText( GameFont *font, UnicodeString text, Color color,
																			 Int x, Int y, Int width, Int height )
{

	/// @todo make all display string rendering go through here!

}  // end WinFormatText

// GameWindowManager::winGetTextSize ==========================================
/** get the extent size of text */
//=============================================================================
void GameWindowManager::winGetTextSize( GameFont *font, UnicodeString text, 
																				Int *width, Int *height, Int maxWidth )
{

	/// @todo make display string size stuff go through here

	if( width )
		*width = 0;

	if( height )
		*height = 0;

}  // end WinGetTextSize

// GameWindowManager::winFontHeight ===========================================
/** Return the font height in pixels */
//=============================================================================
Int GameWindowManager::winFontHeight( GameFont *font )
{
	
	return font->height;

}  // end WinFontHeight

// GameWindowManager::winIsDigit ==============================================
/** You implementation of whether or not character is a digit */
//=============================================================================
Int GameWindowManager::winIsDigit( Int c )
{

	return GameIsDigit( c );

}  // end WinIsDigit

// GameWindowManager::winIsAscii ==============================================
/** You implementation of whether or not character is ascii */
//=============================================================================
Int GameWindowManager::winIsAscii( Int c )
{

	return GameIsAscii( c );

}  // end WinIsDigit

// GameWindowManager::winIsAlNum ==============================================
/** Your implementation of whether or not character is alpha numeric */
//=============================================================================
Int GameWindowManager::winIsAlNum( Int c )
{

	return GameIsAlNum( c );

}  // end WinIsAlNum

// GameWindowManager::winFindFont =============================================
/** Get a font */
//=============================================================================
GameFont *GameWindowManager::winFindFont( AsciiString fontName, 
																					Int pointSize, 
																					Bool bold )
{

	assert( TheFontLibrary );
	if( TheFontLibrary )
		return TheFontLibrary->getFont( fontName, pointSize, bold );

	return NULL;

}  // end WinFindFont

