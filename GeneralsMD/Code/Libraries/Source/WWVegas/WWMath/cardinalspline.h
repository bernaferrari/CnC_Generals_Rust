#if defined(_MSC_VER)
#pragma once
#endif

#ifndef CARDINALSPLINE_H
#define CARDINALSPLINE_H

#include "hermitespline.h"

/**
** CardinalSpline3DClass
** 3-Dimensional cardinal splines
*/
class CardinalSpline3DClass : public HermiteSpline3DClass
{
public:

	virtual int			Add_Key(const Vector3 & point,float t);
	virtual void		Remove_Key(int i);
	virtual void		Clear_Keys(void);

	virtual void		Set_Tightness(int i,float tightness);
	virtual float		Get_Tightness(int i);

	virtual void		Update_Tangents(void);

	// save-load support
	virtual const PersistFactoryClass &	Get_Factory(void) const;
	virtual bool								Save(ChunkSaveClass &csave);
	virtual bool								Load(ChunkLoadClass &cload);

protected:	
	
	DynamicVectorClass<float>	Tightness;
};


/**
** CardinalSpline1DClass
** 1-Dimensional cardinal splines
*/
class CardinalSpline1DClass : public HermiteSpline1DClass
{
public:

	virtual int			Add_Key(float point,float t);
	virtual void		Remove_Key(int i);
	virtual void		Clear_Keys(void);

	virtual void		Set_Tightness(int i,float tightness);
	virtual float		Get_Tightness(int i);

	virtual void		Update_Tangents(void);

	// save-load support
	virtual const PersistFactoryClass &	Get_Factory(void) const;
	virtual bool								Save(ChunkSaveClass &csave);
	virtual bool								Load(ChunkLoadClass &cload);

protected:	
	
	DynamicVectorClass<float>	Tightness;
};


#endif

