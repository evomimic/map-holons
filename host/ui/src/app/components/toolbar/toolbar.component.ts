import { Component, OnInit } from '@angular/core';
import { CommonModule } from '@angular/common';
import { ActivatedRoute, NavigationEnd, Router, RouterModule } from '@angular/router';
import { filter, distinctUntilChanged } from 'rxjs/operators';

export interface Breadcrumb {
  label: string;
  url: string;
}

@Component({
  selector: 'app-toolbar',
  standalone: true,
  imports: [CommonModule, RouterModule],
  templateUrl: './toolbar.component.html',
})
export class ToolbarComponent implements OnInit {
  public breadcrumbs: Breadcrumb[];

  constructor(
    private router: Router,
    private activatedRoute: ActivatedRoute,
  ) {
    this.breadcrumbs = this.buildBreadcrumbs(this.activatedRoute.root);
  }

  ngOnInit() {
    this.router.events.pipe(
      filter((event) => event instanceof NavigationEnd),
      distinctUntilChanged(),
    ).subscribe(() => {
      this.breadcrumbs = this.buildBreadcrumbs(this.activatedRoute.root);
    });
  }

  buildBreadcrumbs(route: ActivatedRoute, url: string = '', breadcrumbs: Breadcrumb[] = []): Breadcrumb[] {
    let label = route.routeConfig && route.routeConfig.data ? route.routeConfig.data['breadcrumb'] : '';
    let path = route.routeConfig && route.routeConfig.path ? route.routeConfig.path : '';
  
    // If the route is a lazy-loaded module, the path is empty, and the label is on the child route
    if (path === '' && route.firstChild) {
      return this.buildBreadcrumbs(route.firstChild, url, breadcrumbs);
    }
  
   // const nextUrl = `${url}${path}/`;
    const nextUrlSegment = [url, path].filter(Boolean).join('/');

    const breadcrumb: Breadcrumb = {
      label: label,
      url: `/${nextUrlSegment}`,
     // url: nextUrl,
    };
  
    //const newBreadcrumbs = breadcrumb.label ? [...breadcrumbs, breadcrumb] : [...breadcrumbs];
    const lastBreadcrumb = breadcrumbs[breadcrumbs.length - 1];
    const newBreadcrumbs = breadcrumb.label && (!lastBreadcrumb || lastBreadcrumb.label !== breadcrumb.label)
      ? [...breadcrumbs, breadcrumb]
      : [...breadcrumbs];
    if (route.firstChild) {
      return this.buildBreadcrumbs(route.firstChild, nextUrlSegment, newBreadcrumbs);
    }
    return newBreadcrumbs;
  }
}