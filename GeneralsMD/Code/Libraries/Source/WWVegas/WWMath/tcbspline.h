#if defined(_MSC_VER)
#pragma once
#endif

#ifndef TCBSPLINE_H
#define TCBSPLINE_H

#include "hermitespline.h"

/*
** TCBSpline3DClass
** Tension-Continuity-Bias splines.  Otherwise known as Kochanek-Bartels cubic splines
*/
class TCBSpline3DClass : public HermiteSpline3DClass
{
public:

	virtual int			Add_Key(const Vector3 & point,float t);
	virtual void		Remove_Key(int i);
	virtual void		Clear_Keys(void);

	virtual void		Set_TCB_Params(int i,float tension,float continuity,float bias);
	virtual void		Get_TCB_Params(int i,float *tension,float *continuity,float *bias);

	void					Update_Tangents(void);

	// save-load support
	virtual const PersistFactoryClass &	Get_Factory(void) const;
	virtual bool								Save(ChunkSaveClass &csave);
	virtual bool								Load(ChunkLoadClass &cload);

protected:	

	class TCBClass
	{
	public:
		float				Tension;
		float				Continuity;
		float				Bias;
		bool				operator == (const TCBClass & that) { return ((Tension == that.Tension) && (Continuity == that.Continuity) && (Bias == that.Bias)); }
		bool				operator != (const TCBClass & that) { return !TCBClass::operator == (that); }
	};

	DynamicVectorClass<TCBClass> Params;
};


#endif

