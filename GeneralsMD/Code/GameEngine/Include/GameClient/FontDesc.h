// FILE: FontDesc.h ///////////////////////////////////////////////////////////////////////////////
// Simple structure used to hold font descriptions.
// Author: Mark Wilczynski, October 2002

#pragma once
#ifndef _FONTDESC_H_
#define _FONTDESC_H_

#include "Common/GameType.h"

struct FontDesc
{
	FontDesc(void);
	AsciiString name;	///<name of font
	Int	size;			///<point size
	Bool bold;			///<is bold?
};

#endif // _FONTDESC_H_
