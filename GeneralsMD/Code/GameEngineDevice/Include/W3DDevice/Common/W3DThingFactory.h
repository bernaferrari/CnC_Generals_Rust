// FILE: W3DThingFactory.h ////////////////////////////////////////////////////////////////////////
// Author: Colin Day, February 2002
// Desc:   Device dependent thing factory access, for things like post processing the
//				 Thing database where we might want to look at device dependent stuff like
//				 model info and such
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3DTHINGFACTORY_H_
#define __W3DTHINGFACTORY_H_

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "Common/ThingFactory.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class W3DThingFactory : public ThingFactory
{

public:

	W3DThingFactory( void );
	virtual ~W3DThingFactory( void );
};  // end W3DThingFactory

#endif // __W3DTHINGFACTORY_H_

