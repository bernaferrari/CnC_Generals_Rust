#ifndef NODEFILT_H
#define NODEFILT_H

#include "always.h"
#include <Max.h>

/***************************************************************
*
*	INodeFilterClass
*
*	This is simply an object used to accept or reject INodes
*	based on whatever criteria you desire.  There are some
*	default node filters defined in this module or you can
*	create your own by inheriting the Abstract Base Class
*	INodeFilterClass and implementing the Accept_Node method.
*
***************************************************************/
class INodeFilterClass 
{
public:
	virtual BOOL Accept_Node(INode * node, TimeValue time) = 0;
};


/***************************************************************
*
*	AnyINodeFilter
*
*	Accepts all INodes...
*
***************************************************************/
class AnyINodeFilter	: public INodeFilterClass
{
public:
	virtual BOOL Accept_Node(INode * node, TimeValue time) { return TRUE; }
};


/***************************************************************
*
*	HelperINodeFilter
*
*	Accepts INodes which are Helper objects 
*
***************************************************************/
class HelperINodeFilter : public INodeFilterClass
{
public:
	virtual BOOL Accept_Node(INode * node, TimeValue time);
};


/***************************************************************
*
*	MeshINodeFilter
*
*	Only accepts INodes which are Triangle meshes 
*
***************************************************************/
class MeshINodeFilter : public INodeFilterClass
{
public:
	virtual BOOL Accept_Node(INode * node, TimeValue time);
};

/***************************************************************
*
*	VisibleMeshINodeFilter
*
*	Only accepts INodes which are Triangle meshes and are
*	currently visible
*
***************************************************************/
class VisibleMeshINodeFilter : public INodeFilterClass
{
public:
	virtual BOOL Accept_Node(INode * node, TimeValue time);
};

/***************************************************************
*
*	VisibleHelperINodeFilter
*
*	Only accepts INodes which are Helper objects and are
*	currently visible
*
***************************************************************/
class VisibleHelperINodeFilter : public INodeFilterClass
{
public:
	virtual BOOL Accept_Node(INode * node, TimeValue time);
};


/***************************************************************
*
*	VisibleMeshOrHelperINodeFilter
*
*	Only accepts INodes which are Triangle meshes or helper
*	objects and are currently visible
*
***************************************************************/
class VisibleMeshOrHelperINodeFilter : public INodeFilterClass
{
public:
	virtual BOOL Accept_Node(INode * node, TimeValue time);
};


/***************************************************************
*
*	AnimatedINodeFilter
*
*	Only accepts INodes which contain at least on animation
*	key.
*
***************************************************************/
class AnimatedINodeFilter : public INodeFilterClass
{
public:
	virtual BOOL Accept_Node(INode * node, TimeValue time);
};


/***************************************************************
*
*	VisibleSelectedINodeFilter
*
*	Only accepts INodes which are Visible and Selected
*
***************************************************************/
class VisibleSelectedINodeFilter : public INodeFilterClass
{
public:
	virtual BOOL Accept_Node(INode * node, TimeValue time);
};



#endif /*NODEFILT_H*/