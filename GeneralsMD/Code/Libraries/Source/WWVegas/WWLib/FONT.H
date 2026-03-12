/*********************************************************************************************** 
 ***              C O N F I D E N T I A L  ---  W E S T W O O D  S T U D I O S               *** 
 *********************************************************************************************** 
 *                                                                                             * 
 *                 Project Name : Command & Conquer                                            * 
 *                                                                                             * 
 *                     $Archive:: /Commando/Code/wwlib/font.h                                 $* 
 *                                                                                             * 
 *                      $Author:: Byon_g                                                      $*
 *                                                                                             * 
 *                     $Modtime:: 11/28/00 2:40p                                              $*
 *                                                                                             * 
 *                    $Revision:: 3                                                           $*
 *                                                                                             *
 *---------------------------------------------------------------------------------------------* 
 * Functions:                                                                                  * 
 * - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - */
#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef FONT_H
#define FONT_H

#include	"convert.h"
#include	"point.h"
#include	"trect.h"
#include	<stddef.h>

class Surface;

/*
**	A font object represent the data that comprises the individual characters as well
**	as drawing text in the font to a surface. This is an abstract base class that is to be
**	derived into a concrete version. 
*/
class FontClass {
	public:
		virtual ~FontClass(void) {}

		virtual int Char_Pixel_Width(char c) const = 0;
		virtual int String_Pixel_Width(char const * string) const = 0;
		virtual int Get_Width(void) const = 0;
		virtual int Get_Height(void) const = 0;
		virtual Point2D Print(char const * string, Surface & surface, Rect const & cliprect, Point2D const & point, ConvertClass const & converter, unsigned char const * remap=NULL) const = 0;

		virtual int Set_XSpacing(int x) = 0;
		virtual int Set_YSpacing(int y) = 0;
};


#endif
