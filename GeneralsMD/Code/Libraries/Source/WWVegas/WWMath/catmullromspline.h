#if defined(_MSC_VER)
#pragma once
#endif

#ifndef CATMULLROMSPLINE_H
#define CATMULLROMSPLINE_H

#include "hermitespline.h"

/**
** CatmullRomSpline3DClass
** This is is an implementation of 3D catmull-rom splines
*/
class CatmullRomSpline3DClass : public HermiteSpline3DClass
{
public:
	void					Update_Tangents(void);

	// save-load support
	virtual const PersistFactoryClass &	Get_Factory(void) const;
	virtual bool								Save(ChunkSaveClass &csave);
	virtual bool								Load(ChunkLoadClass &cload);
};


/**
** CatmullRomSpline1DClass
** This is is an implementation of 1D catmull-rom splines
*/
class CatmullRomSpline1DClass : public HermiteSpline1DClass
{
public:
	void					Update_Tangents(void); 

	// save-load support
	virtual const PersistFactoryClass &	Get_Factory(void) const;
	virtual bool								Save(ChunkSaveClass &csave);
	virtual bool								Load(ChunkLoadClass &cload);
};

#endif