#if defined(_MSC_VER)
#pragma once
#endif

#ifndef LAYER_H
#define LAYER_H

#include "always.h"
#include "listnode.h"
#include "vector3.h"

class SceneClass;
class CameraClass;

class LayerClass;
typedef Node<LayerClass *> LayerNodeClass;

class LayerClass : public LayerNodeClass
{

public:

	LayerClass(void);
	LayerClass(SceneClass * s,CameraClass * c,bool clear = false,bool clearz = false,const Vector3 & color = Vector3(0,0,0)); 
	LayerClass(const LayerClass & src);
	~LayerClass(void);


	/*
	** The following functions will handle the references of the Scene and Camera
	** objects properly.
	*/
	void						Set_Scene(SceneClass * scene);
	SceneClass *			Get_Scene(void) const;
	SceneClass *			Peek_Scene(void) const;
	void						Set_Camera(CameraClass * cam);
	CameraClass *			Get_Camera(void) const;
	CameraClass *			Peek_Camera(void) const;


	// [SKB: Aug 14 2001 @ 1:53pm] :
	// Add a method to copy one layer to another - I would like to create an assignment
	// operator but it could break old code.
	void						Set(const LayerClass & layer);

	/*
	** LayerClass members are public since this is a "lightweight" class
	*/
	bool						Clear;
	bool						ClearZ;
	Vector3					ClearColor;

	SceneClass *			Scene;
	CameraClass *			Camera;

};

typedef List<LayerClass *> LayerListClass;


#endif //LAYER_H


