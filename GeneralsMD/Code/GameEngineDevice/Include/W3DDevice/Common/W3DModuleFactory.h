// FILE: W3DModuleFactory.h ///////////////////////////////////////////////////////////////////////
// Author: Steven Johnson, September 2001
//				 Colin Day, November 2001
// Desc:   W3D game logic class, there shouldn't be a lot of new functionality
//				 in this class, but there are certain things that need to have close 
//				 knowledge of each other like ther logical and visual terrain
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3DMODULEFACTORY_H_
#define __W3DMODULEFACTORY_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/ModuleFactory.h"

//-------------------------------------------------------------------------------------------------
/** W3D specific functionality for the module factory */
//-------------------------------------------------------------------------------------------------
class W3DModuleFactory : public ModuleFactory
{

public:

	virtual void init( void );  
	
};

#endif // __W3DMODULEFACTORY_H_
