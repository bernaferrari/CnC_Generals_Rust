// FILE: W3DDependencyModelDraw.h ////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, October 2002
// Desc: Draw module just like Model, except it can't draw unless somebody else explicitly says to, since they
// have to draw first.
//
// This draw module can be used in a general case (although I don't see why), m_attachToDrawableBoneInContainer
// is for the one present and main reason to use this module.  Our transport needs to tell us it is okay to
// draw after he draws.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _W3D_DEPENDENCY_MODEL_DRAW_H_
#define _W3D_DEPENDENCY_MODEL_DRAW_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "W3DDevice/GameClient/Module/W3DModelDraw.h"

//-------------------------------------------------------------------------------------------------
class W3DDependencyModelDrawModuleData : public W3DModelDrawModuleData
{
public:
	AsciiString	m_attachToDrawableBoneInContainer;// Not just a different draw module, this bone is in our container

	W3DDependencyModelDrawModuleData();
	~W3DDependencyModelDrawModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);
};

//-------------------------------------------------------------------------------------------------
class W3DDependencyModelDraw : public W3DModelDraw
{

 	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( W3DDependencyModelDraw, "W3DDependencyModelDraw" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( W3DDependencyModelDraw, W3DDependencyModelDrawModuleData )
		
public:

	W3DDependencyModelDraw( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration
	virtual void doDrawModule(const Matrix3D* transformMtx);
	virtual void notifyDrawModuleDependencyCleared( );///< if you were waiting for something before you drew, it's ready now
	virtual void adjustTransformMtx(Matrix3D& mtx) const;

protected:
	Bool m_dependencyCleared; // The thing we depend on will clear this, and we will relatch it after we draw.
};

#endif // _W3D_DEPENDENCY_MODEL_DRAW_H_

