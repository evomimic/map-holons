import { Component, EventEmitter, Output } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { Holon } from '../../models/holon';

export type NewHolon = {
    title: '',
    description: '',
    visibility: 'private',
  };

@Component({
  selector: 'app-holon-form',
  standalone: true,
  imports: [CommonModule, FormsModule],
  templateUrl: './holon-form.component.html',
})
export class HolonFormComponent {
  // Outputs to notify the parent component of user actions
  @Output() formClosed = new EventEmitter<void>();
  @Output() holonCreated = new EventEmitter<NewHolon>();

  // The local state for the form's data model
 newHolon: NewHolon = {
    title: '',
    description: '',
    visibility: 'private',
  };

  closeForm(): void {
    this.formClosed.emit();
  }

  onSubmit(): void {
    // Emit the new holon data to the parent
    this.holonCreated.emit(this.newHolon);
    // Reset the form for the next time it's opened
    this.newHolon = { title: '', description: '', visibility: 'private' };
  }
}