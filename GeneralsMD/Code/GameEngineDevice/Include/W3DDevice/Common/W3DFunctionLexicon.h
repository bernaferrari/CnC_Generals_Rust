// FILE: W3DFunctionLexicon.h /////////////////////////////////////////////////////////////////////
// Created:    Colin Day, September 2001
// Desc:       Function lexicon for w3d specific funtion pointers
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3DFUNCTIONLEXICON_H_
#define __W3DFUNCTIONLEXICON_H_

#include "Common/FunctionLexicon.h"

//-------------------------------------------------------------------------------------------------
/** The function lexicon that can also contain w3d device methods */
//-------------------------------------------------------------------------------------------------
class W3DFunctionLexicon : public FunctionLexicon
{

public:

	W3DFunctionLexicon( void );
	virtual ~W3DFunctionLexicon( void );

	virtual void init( void );
	virtual void reset( void );
	virtual void update( void );
	
protected:

};

#endif // __W3DFUNCTIONLEXICON_H_

