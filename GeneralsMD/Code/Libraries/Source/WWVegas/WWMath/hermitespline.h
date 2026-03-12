#if defined(_MSC_VER)
#pragma once
#endif

#ifndef HERMITE_H
#define HERMITE_H

#include "curve.h"

/*
** HermiteSpline3DClass
** 3-Dimensional hermite spline interpolation
** Hermite splines require you to input all of the tangents...
*/
class HermiteSpline3DClass : public Curve3DClass
{
public:
	HermiteSpline3DClass(void)
		: TangentsDirty (true) { }

	HermiteSpline3DClass(const HermiteSpline3DClass &that)
		: TangentsDirty (true) { (*this) = that; }

	const HermiteSpline3DClass &operator= (const HermiteSpline3DClass &that);
	
	virtual void		Evaluate(float time,Vector3 * set_val);
	virtual void		Evaluate_Derivative(float time,Vector3 * set_val);
	virtual void		Set_Looping(bool onoff);
	
	virtual void		Set_Key(int i,const Vector3 & point);
	virtual int			Add_Key(const Vector3 & point,float t);
	virtual void		Remove_Key(int i);
	virtual void		Clear_Keys(void);

	virtual void		Set_Tangents(int i,const Vector3 & in_tan,const Vector3 & out_tan);
	virtual void		Get_Tangents(int i,Vector3 * set_in,Vector3 * set_out);

	virtual void		Update_Tangents(void) { TangentsDirty = false; }

	// save-load support
	virtual const PersistFactoryClass &	Get_Factory(void) const;
	virtual bool								Save(ChunkSaveClass &csave);
	virtual bool								Load(ChunkLoadClass &cload);

protected:

	class TangentsClass
	{
	public:
		Vector3			InTangent;
		Vector3			OutTangent;
		bool				operator == (const TangentsClass & that) { return ((InTangent == that.InTangent) && (OutTangent == that.OutTangent)); }
		bool				operator != (const TangentsClass & that) { return !TangentsClass::operator == (that); }
	};	

	bool											TangentsDirty;
	DynamicVectorClass<TangentsClass>	Tangents;
};


/*
** HermiteSpline1DClass
** 1-Dimensional hermite spline interpolation
** Hermite splines require you to input all of the tangents...
*/
class HermiteSpline1DClass : public Curve1DClass
{
public:
	HermiteSpline1DClass (void)
		: TangentsDirty (true) { }
	
	virtual void		Evaluate(float time,float * set_val);
	virtual void		Set_Looping(bool onoff);
	
	virtual void		Set_Key(int i,float point,unsigned int extra=0);
	virtual int			Add_Key(float point,float t,unsigned int extra=0);
	virtual void		Remove_Key(int i);
	virtual void		Clear_Keys(void);

	virtual void		Set_Tangents(int i,float in_tan,float out_tan);
	virtual void		Get_Tangents(int i,float * set_in,float * set_out);

	// save-load support
	virtual const PersistFactoryClass &	Get_Factory(void) const;
	virtual bool								Save(ChunkSaveClass &csave);
	virtual bool								Load(ChunkLoadClass &cload);
	
protected:

	class TangentsClass
	{
	public:
		float			InTangent;
		float			OutTangent;
		bool				operator == (const TangentsClass & that) { return ((InTangent == that.InTangent) && (OutTangent == that.OutTangent)); }
		bool				operator != (const TangentsClass & that) { return !TangentsClass::operator == (that); }
	};

	virtual void		Update_Tangents(void) { TangentsDirty = false; }

	bool											TangentsDirty;
	DynamicVectorClass<TangentsClass>	Tangents;
};


#endif
