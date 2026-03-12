// FILE: GUIEditColor.h ///////////////////////////////////////////////////////
//
// Project:    GUIEdit
//
// File name:  GUIEditColor.h
//
// Created:    Colin Day, July 2001
//
// Desc:       Color structures for the editor
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __GUIEDITCOLOR_H_
#define __GUIEDITCOLOR_H_

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////

// FORWARD REFERENCES /////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// TYPE DEFINES ///////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////

// RGBColorInt ----------------------------------------------------------------
/** Integer color representation */
//-----------------------------------------------------------------------------
struct RGBColorInt
{
	Int red;
	Int green;
	Int blue;
	Int alpha;
};  // end RGBColorInt

// RGBColorReal ---------------------------------------------------------------
/** Colors using 0.0 to 1.0 reals */
//-----------------------------------------------------------------------------
struct RGBColorReal
{
	Real red;
	Real green;
	Real blue;
	Real alpha;
};  // end RGBColorReal

// HSVColorReal ---------------------------------------------------------------
/** Colors using hue, saturation, value using 0.0 to 1.0 reals */
//-----------------------------------------------------------------------------
struct HSVColorReal
{
  Real hue;
  Real saturation;
  Real value;
	Real alpha;
};  // end HSVReal

// INLINING ///////////////////////////////////////////////////////////////////

// EXTERNALS //////////////////////////////////////////////////////////////////
extern RGBColorInt *SelectColor( Int red, Int green, Int blue, Int alpha,
																 Int mouseX = 0, Int mouseY = 0 );

#endif // __GUIEDITCOLOR_H_

