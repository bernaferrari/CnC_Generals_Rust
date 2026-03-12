// FILE: W3DScienceModelDraw.h ////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, NOVEMBER 2002
// Desc: Draw module just like Model, except it only draws if the local player has the specified science
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _W3D_SCIENCE_MODEL_DRAW_H_
#define _W3D_SCIENCE_MODEL_DRAW_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "W3DDevice/GameClient/Module/W3DModelDraw.h"

enum ScienceType;

//-------------------------------------------------------------------------------------------------
class W3DScienceModelDrawModuleData : public W3DModelDrawModuleData
{
public:
	ScienceType m_requiredScience; ///< Local player must have this science for me to ever draw

	W3DScienceModelDrawModuleData();
	~W3DScienceModelDrawModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);
};

//-------------------------------------------------------------------------------------------------
class W3DScienceModelDraw : public W3DModelDraw
{

 	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( W3DScienceModelDraw, "W3DScienceModelDraw" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( W3DScienceModelDraw, W3DScienceModelDrawModuleData )
		
public:

	W3DScienceModelDraw( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration
	virtual void doDrawModule(const Matrix3D* transformMtx);///< checks a property on the local player before passing this up

protected:
};

#endif

