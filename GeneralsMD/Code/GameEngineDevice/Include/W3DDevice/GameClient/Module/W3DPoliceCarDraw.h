// FILE: W3DPoliceCarDraw.h ///////////////////////////////////////////////////////////////////////
// Author: 
// Desc:   
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3DPOLICECARDRAW_H_
#define __W3DPOLICECARDRAW_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/DrawModule.h"
#include "W3DDevice/GameClient/Module/W3DTruckDraw.h"
#include "W3DDevice/GameClient/W3DDynamicLight.h"
#include "WW3D2/Line3D.h"

//-------------------------------------------------------------------------------------------------
/** W3D police car draw */
//-------------------------------------------------------------------------------------------------
class W3DPoliceCarDraw : public W3DTruckDraw
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( W3DPoliceCarDraw, "W3DPoliceCarDraw" )
	MAKE_STANDARD_MODULE_MACRO( W3DPoliceCarDraw )

public:

	W3DPoliceCarDraw( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void doDrawModule(const Matrix3D* transformMtx);

protected:

	/// create the dynamic light for the search light
	W3DDynamicLight *createDynamicLight( void );

	W3DDynamicLight *m_light;  ///< light for the POLICECAR
	Real					m_curFrame;

};

#endif // __W3DPOLICECARDRAW_H_

